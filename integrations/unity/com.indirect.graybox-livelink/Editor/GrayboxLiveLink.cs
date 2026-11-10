// Graybox Live Link: reads the screens Graybox exports (one folder per screen: layout.json and
// sprites/*.png) and builds them as uGUI under a Canvas: RectTransforms anchored as the layers'
// constraints say, Images (9-sliced where Graybox 9-slices), and texts. With "Watch" on it rebuilds a
// screen each time Graybox exports it again (Graybox exports the screens marked for Unity each time
// the file is saved). Objects it built keep a GrayboxNode, so an update changes them in place:
// components you add to them stay.
using System;
using System.Collections.Generic;
using System.IO;
using UnityEditor;
using UnityEngine;
using UnityEngine.UI;
#if GRAYBOX_TMP
using TMPro;
#endif

namespace Indirect.Graybox.Editor
{
    [Serializable]
    class Anchors
    {
        public string h = "left";
        public string v = "top";
    }

    [Serializable]
    class TextData
    {
        public string characters = "";
        public string fontFamily = "";
        public string fontStyle = "";
        public float fontSize = 14f;
        public string color = "";
        public string align = "left";
        public string alignVertical = "top";
    }

    [Serializable]
    class NodeData
    {
        public long id;
        public long parent;
        public string name = "";
        public string type = "";
        public float x, y, width, height;
        public float opacity = 1f;
        public Anchors anchors = new Anchors();
        public string sprite = "";
        public float[] border = new float[0];
        public TextData text = new TextData();
    }

    [Serializable]
    class Layout
    {
        public int graybox;
        public string name = "";
        public long id;
        public float width, height;
        public float scale = 1f;
        public NodeData[] nodes = new NodeData[0];
    }

    public sealed class GrayboxLiveLink : EditorWindow
    {
        const string FolderPref = "Indirect.Graybox.LiveLink.Folder";
        const string WatchPref = "Indirect.Graybox.LiveLink.Watch";
        const string AssetRoot = "Assets/Graybox";

        string folder = "";
        bool watch;
        Canvas canvas;
        FileSystemWatcher watcher;
        // Set by the watcher's thread, read on the editor's.
        volatile bool changed;
        double changedAt;
        string status = "";

        [MenuItem("Tools/Graybox Live Link")]
        static void Open() => GetWindow<GrayboxLiveLink>("Graybox Live Link");

        void OnEnable()
        {
            folder = EditorPrefs.GetString(FolderPref, "");
            watch = EditorPrefs.GetBool(WatchPref, false);
            EditorApplication.update += Tick;
            Rewatch();
        }

        void OnDisable()
        {
            EditorApplication.update -= Tick;
            StopWatching();
        }

        void OnGUI()
        {
            EditorGUILayout.LabelField("Graybox export folder", EditorStyles.boldLabel);
            EditorGUILayout.BeginHorizontal();
            var picked = EditorGUILayout.TextField(folder);
            if (GUILayout.Button("Browse…", GUILayout.Width(80)))
            {
                var p = EditorUtility.OpenFolderPanel("Graybox export folder", folder, "");
                if (!string.IsNullOrEmpty(p)) picked = p;
            }
            EditorGUILayout.EndHorizontal();
            if (picked != folder)
            {
                folder = picked;
                EditorPrefs.SetString(FolderPref, folder);
                Rewatch();
            }
            canvas = (Canvas)EditorGUILayout.ObjectField("Canvas (optional)", canvas, typeof(Canvas), true);
            var w = EditorGUILayout.Toggle("Watch (rebuild on export)", watch);
            if (w != watch)
            {
                watch = w;
                EditorPrefs.SetBool(WatchPref, watch);
                Rewatch();
            }
            EditorGUILayout.Space();
            using (new EditorGUI.DisabledScope(!Directory.Exists(folder)))
            {
                if (GUILayout.Button("Import now")) ImportAll();
            }
            foreach (var screen in Screens()) EditorGUILayout.LabelField("• " + Path.GetFileName(screen));
            if (!string.IsNullOrEmpty(status)) EditorGUILayout.HelpBox(status, MessageType.Info);
        }

        /// The screen folders in the export folder (those with a layout.json).
        IEnumerable<string> Screens()
        {
            if (!Directory.Exists(folder)) yield break;
            foreach (var d in Directory.GetDirectories(folder))
                if (File.Exists(Path.Combine(d, "layout.json"))) yield return d;
        }

        void Rewatch()
        {
            StopWatching();
            if (!watch || !Directory.Exists(folder)) return;
            watcher = new FileSystemWatcher(folder, "layout.json") { IncludeSubdirectories = true, NotifyFilter = NotifyFilters.LastWrite | NotifyFilters.FileName };
            FileSystemEventHandler on = (_, __) => changed = true;
            watcher.Changed += on;
            watcher.Created += on;
            watcher.EnableRaisingEvents = true;
        }

        void StopWatching()
        {
            if (watcher == null) return;
            watcher.EnableRaisingEvents = false;
            watcher.Dispose();
            watcher = null;
        }

        void Tick()
        {
            if (!changed) return;
            // Wait a moment: Graybox writes the sprites and layout.json one after another.
            if (changedAt == 0) changedAt = EditorApplication.timeSinceStartup;
            if (EditorApplication.timeSinceStartup - changedAt < 0.5) return;
            changed = false;
            changedAt = 0;
            ImportAll();
            Repaint();
        }

        /// Import every screen in the export folder.
        public void ImportAll()
        {
            int n = 0;
            foreach (var screen in Screens())
            {
                try
                {
                    Import(screen, ref canvas);
                    n++;
                }
                catch (Exception e)
                {
                    Debug.LogError($"Graybox Live Link: {Path.GetFileName(screen)}: {e.Message}");
                }
            }
            status = $"Imported {n} screen{(n == 1 ? "" : "s")} at {DateTime.Now:HH:mm:ss}";
        }

        /// Build (or update) the screen in folder `dir` under `canvas` (made when there is none).
        public static void Import(string dir, ref Canvas canvas)
        {
            var layout = JsonUtility.FromJson<Layout>(File.ReadAllText(Path.Combine(dir, "layout.json")));
            if (layout == null || layout.nodes == null || layout.nodes.Length == 0) return;
            var assets = $"{AssetRoot}/{Path.GetFileName(dir)}";
            Directory.CreateDirectory(assets);
            var sprites = ImportSprites(dir, assets, layout);
            if (canvas == null) canvas = FindOrMakeCanvas(layout);
            var root = layout.nodes[0];
            var byId = new Dictionary<long, NodeData>();
            foreach (var n in layout.nodes) byId[n.id] = n;
            // What this screen built last time, by layer.
            var existing = new Dictionary<long, GrayboxNode>();
            foreach (var g in canvas.GetComponentsInChildren<GrayboxNode>(true))
                if (g.screenId == layout.id) existing[g.layerId] = g;
            var built = new Dictionary<long, RectTransform>();
            var seen = new HashSet<long>();
            int order = 0;
            foreach (var n in layout.nodes)
            {
                var parent = n.id == root.id ? (RectTransform)canvas.transform : (built.TryGetValue(n.parent, out var p) ? p : null);
                if (parent == null) continue;
                var go = existing.TryGetValue(n.id, out var had) ? had.gameObject : NewObject(n, layout.id);
                go.name = n.name;
                var rt = go.GetComponent<RectTransform>();
                if (rt.parent != parent) rt.SetParent(parent, false);
                if (n.id != root.id) rt.SetSiblingIndex(order++);
                var parentData = n.id == root.id ? null : byId.TryGetValue(n.parent, out var pd) ? pd : null;
                Place(rt, n, parentData);
                Look(go, n, sprites);
                built[n.id] = rt;
                seen.Add(n.id);
            }
            // Layers gone from the design go from the scene too.
            foreach (var kv in existing)
                if (!seen.Contains(kv.Key) && kv.Value != null) UnityEngine.Object.DestroyImmediate(kv.Value.gameObject);
            EditorUtility.SetDirty(canvas.gameObject);
        }

        static GameObject NewObject(NodeData n, long screen)
        {
            var go = new GameObject(n.name, typeof(RectTransform));
            Undo.RegisterCreatedObjectUndo(go, "Graybox import");
            var g = go.AddComponent<GrayboxNode>();
            g.layerId = n.id;
            g.screenId = screen;
            return go;
        }

        static Canvas FindOrMakeCanvas(Layout layout)
        {
            var c = UnityEngine.Object.FindObjectOfType<Canvas>();
            if (c != null) return c;
            var go = new GameObject("Canvas", typeof(Canvas), typeof(CanvasScaler), typeof(GraphicRaycaster));
            Undo.RegisterCreatedObjectUndo(go, "Graybox canvas");
            c = go.GetComponent<Canvas>();
            c.renderMode = RenderMode.ScreenSpaceOverlay;
            var scaler = go.GetComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(Mathf.Max(1, layout.width), Mathf.Max(1, layout.height));
            scaler.matchWidthOrHeight = 0.5f;
            return c;
        }

        /// Copy the screen's sprites into the project as 9-sliced (where they have a border) sprites.
        static Dictionary<string, Sprite> ImportSprites(string dir, string assets, Layout layout)
        {
            var map = new Dictionary<string, Sprite>();
            // Copy them all, then let the project see the folder and its files once.
            var copied = new List<(NodeData node, string path)>();
            foreach (var n in layout.nodes)
            {
                if (string.IsNullOrEmpty(n.sprite)) continue;
                var src = Path.Combine(dir, n.sprite);
                if (!File.Exists(src)) continue;
                var dst = $"{assets}/{Path.GetFileName(n.sprite)}";
                File.Copy(src, dst, true);
                copied.Add((n, dst));
            }
            AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport);
            foreach (var (n, dst) in copied)
            {
                if (AssetImporter.GetAtPath(dst) is TextureImporter ti)
                {
                    ti.textureType = TextureImporterType.Sprite;
                    ti.spriteImportMode = SpriteImportMode.Single;
                    ti.mipmapEnabled = false;
                    ti.alphaIsTransparency = true;
                    ti.npotScale = TextureImporterNPOTScale.None;
                    // One design unit is one canvas unit at the canvas' 100 pixels per unit.
                    ti.spritePixelsPerUnit = 100f * Mathf.Max(0.01f, layout.scale);
                    var b = n.border;
                    ti.spriteBorder = b != null && b.Length == 4 ? new Vector4(b[0], b[1], b[2], b[3]) : Vector4.zero;
                    ti.SaveAndReimport();
                }
                var s = AssetDatabase.LoadAssetAtPath<Sprite>(dst);
                if (s != null) map[n.sprite] = s;
            }
            return map;
        }

        /// Anchors from the layer's constraints, and offsets that put it where it is in its parent.
        static void Place(RectTransform rt, NodeData n, NodeData parent)
        {
            rt.localScale = Vector3.one;
            rt.localRotation = Quaternion.identity;
            rt.pivot = new Vector2(0.5f, 0.5f);
            if (parent == null)
            {
                rt.anchorMin = Vector2.zero;
                rt.anchorMax = Vector2.one;
                rt.offsetMin = Vector2.zero;
                rt.offsetMax = Vector2.zero;
                return;
            }
            float pw = Mathf.Max(0.001f, parent.width), ph = Mathf.Max(0.001f, parent.height);
            // Unity measures up from the bottom; Graybox down from the top.
            float xMin = n.x, xMax = n.x + n.width, yMin = ph - (n.y + n.height), yMax = ph - n.y;
            Vector2 aMin, aMax;
            switch (n.anchors.h)
            {
                case "right": aMin.x = aMax.x = 1f; break;
                case "center": aMin.x = aMax.x = 0.5f; break;
                case "stretch": aMin.x = 0f; aMax.x = 1f; break;
                case "scale": aMin.x = xMin / pw; aMax.x = xMax / pw; break;
                default: aMin.x = aMax.x = 0f; break;
            }
            switch (n.anchors.v)
            {
                case "bottom": aMin.y = aMax.y = 0f; break;
                case "center": aMin.y = aMax.y = 0.5f; break;
                case "stretch": aMin.y = 0f; aMax.y = 1f; break;
                case "scale": aMin.y = yMin / ph; aMax.y = yMax / ph; break;
                default: aMin.y = aMax.y = 1f; break;
            }
            rt.anchorMin = aMin;
            rt.anchorMax = aMax;
            rt.offsetMin = new Vector2(xMin - aMin.x * pw, yMin - aMin.y * ph);
            rt.offsetMax = new Vector2(xMax - aMax.x * pw, yMax - aMax.y * ph);
        }

        /// The layer's sprite (an Image, sliced when it has a border) or its text.
        static void Look(GameObject go, NodeData n, Dictionary<string, Sprite> sprites)
        {
            if (n.type == "text")
            {
                SetText(go, n);
                return;
            }
            if (!string.IsNullOrEmpty(n.sprite) && sprites.TryGetValue(n.sprite, out var sprite))
            {
                var img = go.GetComponent<Image>();
                if (img == null) img = go.AddComponent<Image>();
                img.sprite = sprite;
                var sliced = n.border != null && n.border.Length == 4 && (n.border[0] > 0 || n.border[1] > 0 || n.border[2] > 0 || n.border[3] > 0);
                img.type = sliced ? Image.Type.Sliced : Image.Type.Simple;
                img.color = new Color(1f, 1f, 1f, Mathf.Clamp01(n.opacity));
                img.raycastTarget = true;
            }
        }

        static Color ParseColor(string hex)
        {
            if (!string.IsNullOrEmpty(hex) && ColorUtility.TryParseHtmlString(hex.StartsWith("#") ? hex : "#" + hex, out var c)) return c;
            return Color.white;
        }

        static void SetText(GameObject go, NodeData n)
        {
            var t = n.text ?? new TextData();
            var color = ParseColor(t.color);
            color.a *= Mathf.Clamp01(n.opacity);
#if GRAYBOX_TMP
            var tmp = go.GetComponent<TextMeshProUGUI>();
            if (tmp == null) tmp = go.AddComponent<TextMeshProUGUI>();
            tmp.text = t.characters;
            tmp.fontSize = t.fontSize;
            tmp.color = color;
            tmp.enableWordWrapping = true;
            tmp.alignment = TmpAlign(t.align, t.alignVertical);
#else
            var txt = go.GetComponent<Text>();
            if (txt == null) txt = go.AddComponent<Text>();
            txt.text = t.characters;
            txt.fontSize = Mathf.RoundToInt(t.fontSize);
            txt.color = color;
            if (txt.font == null) txt.font = Resources.GetBuiltinResource<Font>("LegacyRuntime.ttf");
            txt.alignment = LegacyAlign(t.align, t.alignVertical);
#endif
        }

#if GRAYBOX_TMP
        static TextAlignmentOptions TmpAlign(string h, string v)
        {
            switch (v + "/" + h)
            {
                case "top/center": return TextAlignmentOptions.Top;
                case "top/right": return TextAlignmentOptions.TopRight;
                case "top/justified": return TextAlignmentOptions.TopJustified;
                case "center/left": return TextAlignmentOptions.Left;
                case "center/center": return TextAlignmentOptions.Center;
                case "center/right": return TextAlignmentOptions.Right;
                case "center/justified": return TextAlignmentOptions.Justified;
                case "bottom/left": return TextAlignmentOptions.BottomLeft;
                case "bottom/center": return TextAlignmentOptions.Bottom;
                case "bottom/right": return TextAlignmentOptions.BottomRight;
                case "bottom/justified": return TextAlignmentOptions.BottomJustified;
                default: return TextAlignmentOptions.TopLeft;
            }
        }
#else
        static TextAnchor LegacyAlign(string h, string v)
        {
            int col = h == "center" ? 1 : h == "right" ? 2 : 0;
            int row = v == "center" ? 1 : v == "bottom" ? 2 : 0;
            return (TextAnchor)(row * 3 + col);
        }
#endif
    }
}
