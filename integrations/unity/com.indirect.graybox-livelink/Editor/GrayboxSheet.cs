// Graybox pixel art sprite sheets. Graybox exports an animated pixel sprite as a PNG sheet and a JSON
// next to it (`pixel.export` with format "unity", or the `<layer id>_anim` files of a screen export).
// This file reads that JSON and
//   - sets the PNG up as a Sprite (Multiple): Point filter, no compression, no mipmaps, the pixels per
//     unit, and one sprite per picture on the sheet, with its pivot and 9-slice border;
//   - makes one AnimationClip per tag (or one, "all", when there are none) that swaps the sprite
//     (`SpriteRenderer` for a sheet dropped in Assets, `Image` for a Live Link screen), and an
//     AnimatorController whose first state plays the sprite's `play` clip.
// A PNG in Assets with a Graybox JSON of the same name next to it (with a `meta.unity` block) is
// handled by GrayboxSheetPostprocessor, so exporting from Graybox into the project is all it takes.
// Delete the JSON to set the PNG up by hand.
using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using UnityEngine.UI;

namespace Indirect.Graybox.Editor
{
    internal static class GrayboxSheet
    {
        // Mirror of the sheet JSON (JsonUtility: public fields, no dictionaries; a missing field keeps
        // its default). Only what Unity needs is read.
        [Serializable]
        internal class Box
        {
            public int x, y, w, h;
        }

        [Serializable]
        internal class Pivot
        {
            public float x = 0.5f;
            public float y = 0.5f;
        }

        // In sheet pixels.
        [Serializable]
        internal class Border
        {
            public float left, bottom, right, top;
        }

        [Serializable]
        internal class Frame
        {
            public string filename = "";
            public Box frame = new Box();
            // Milliseconds.
            public int duration = 100;
            // From the picture's top left, as Graybox draws.
            public Pivot pivot = new Pivot();
            public Border border = new Border();
        }

        [Serializable]
        internal class Tag
        {
            public string name = "";
            // Indices into `frames`, both included.
            public int from, to;
            // forward | reverse | pingpong
            public string direction = "forward";
            // 0: loops forever.
            public int repeat;
        }

        [Serializable]
        internal class UnityMeta
        {
            public string spriteMode = "";
            public string filterMode = "Point";
            public string compression = "None";
            public float pixelsPerUnit;
            // The tag that plays first (null in the JSON: the first clip).
            public string play = "";
        }

        [Serializable]
        internal class Meta
        {
            public string app = "";
            public string image = "";
            public Box size = new Box();
            public Tag[] frameTags = new Tag[0];
            public UnityMeta unity = new UnityMeta();
        }

        [Serializable]
        internal class Data
        {
            public Frame[] frames = new Frame[0];
            public Meta meta = new Meta();

            /// Graybox wrote a `meta.unity` block: it wants this sheet imported for Unity.
            public bool ForUnity => meta != null && meta.unity != null && (!string.IsNullOrEmpty(meta.unity.spriteMode) || meta.unity.pixelsPerUnit > 0f);
        }

        /// Read a sheet JSON; null (and a warning) when it is not one.
        internal static Data Parse(string text, string what)
        {
            try
            {
                var d = JsonUtility.FromJson<Data>(text);
                if (d != null && d.frames != null && d.meta != null) return d;
            }
            catch (Exception e)
            {
                Debug.LogWarning($"Graybox: {what} is not a readable sprite sheet ({e.Message})");
                return null;
            }
            Debug.LogWarning($"Graybox: {what} is not a readable sprite sheet");
            return null;
        }

        /// The Graybox sheet JSON that sits next to the PNG at `pngPath` (same name), or null: no JSON,
        /// not Graybox's, or no `meta.unity` block.
        internal static Data ForTexture(string pngPath)
        {
            try
            {
                var jsonPath = Path.ChangeExtension(pngPath, ".json");
                if (!File.Exists(jsonPath)) return null;
                var text = File.ReadAllText(jsonPath);
                // Cheap test first: most JSON files in a project are not ours.
                if (!text.Contains("\"Graybox\"")) return null;
                var d = Parse(text, jsonPath);
                return d != null && d.meta.app == "Graybox" && d.ForUnity ? d : null;
            }
            catch (Exception e)
            {
                Debug.LogWarning($"Graybox: the sheet next to {pngPath} could not be read ({e.Message})");
                return null;
            }
        }

        /// The size of a PNG, from its header.
        internal static bool PngSize(string path, out int w, out int h)
        {
            w = h = 0;
            try
            {
                using (var fs = File.OpenRead(path))
                {
                    var b = new byte[24];
                    int got = 0;
                    while (got < b.Length)
                    {
                        int n = fs.Read(b, got, b.Length - got);
                        if (n <= 0) return false;
                        got += n;
                    }
                    if (b[0] != 0x89 || b[1] != (byte)'P' || b[2] != (byte)'N' || b[3] != (byte)'G') return false;
                    w = (b[16] << 24) | (b[17] << 16) | (b[18] << 8) | b[19];
                    h = (b[20] << 24) | (b[21] << 16) | (b[22] << 8) | b[23];
                    return w > 0 && h > 0;
                }
            }
            catch (Exception)
            {
                return false;
            }
        }

        /// One sprite per distinct picture on the sheet (frames that share a cell share a sprite), named
        /// `<name>_<k>` after the first frame k that uses it. `nameOfFrame` says which sprite a frame is.
        internal static SpriteMetaData[] Slices(Data d, string name, int texW, int texH, out Dictionary<int, string> nameOfFrame)
        {
            nameOfFrame = new Dictionary<int, string>();
            var seen = new Dictionary<(int, int, int, int), string>();
            var list = new List<SpriteMetaData>();
            for (int k = 0; k < d.frames.Length; k++)
            {
                var f = d.frames[k];
                if (f == null || f.frame == null) continue;
                // Keep the rectangle on the sheet.
                int x = Mathf.Clamp(f.frame.x, 0, texW - 1), y = Mathf.Clamp(f.frame.y, 0, texH - 1);
                int w = Mathf.Min(f.frame.w, texW - x), h = Mathf.Min(f.frame.h, texH - y);
                if (w <= 0 || h <= 0) continue;
                var key = (x, y, w, h);
                if (!seen.TryGetValue(key, out var spriteName))
                {
                    spriteName = $"{name}_{k}";
                    seen[key] = spriteName;
                    var p = f.pivot ?? new Pivot();
                    var b = f.border ?? new Border();
                    list.Add(new SpriteMetaData
                    {
                        name = spriteName,
                        // Graybox measures down from the top, Unity up from the bottom.
                        rect = new Rect(x, texH - y - h, w, h),
                        alignment = (int)SpriteAlignment.Custom,
                        pivot = new Vector2(Finite(p.x, 0.5f), 1f - Finite(p.y, 0.5f)),
                        // Unity's order: left, bottom, right, top.
                        border = new Vector4(Mathf.Clamp(b.left, 0, w), Mathf.Clamp(b.bottom, 0, h), Mathf.Clamp(b.right, 0, w), Mathf.Clamp(b.top, 0, h)),
                    });
                }
                nameOfFrame[k] = spriteName;
            }
            return list.ToArray();
        }

        static float Finite(float v, float fallback) => float.IsNaN(v) || float.IsInfinity(v) ? fallback : v;

        /// Set the importer up as the sheet asks. `ppu`: pixels per unit (the JSON's when null).
        /// Does not save: call it before an import (a preprocessor) or follow it with SaveAndReimport.
        internal static void Configure(TextureImporter ti, Data d, string pngPath, float? ppu)
        {
            var u = d.meta.unity ?? new UnityMeta();
            ti.textureType = TextureImporterType.Sprite;
            ti.mipmapEnabled = false;
            ti.alphaIsTransparency = true;
            ti.npotScale = TextureImporterNPOTScale.None;
            ti.wrapMode = TextureWrapMode.Clamp;
            ti.filterMode = Enum.TryParse<FilterMode>(u.filterMode, true, out var fm) ? fm : FilterMode.Point;
            ti.textureCompression = Compression(u.compression);
            float pixels = ppu ?? (u.pixelsPerUnit > 0f ? u.pixelsPerUnit : 100f);
            ti.spritePixelsPerUnit = Finite(pixels, 100f) > 0f ? Finite(pixels, 100f) : 100f;
            int texW = d.meta.size != null ? d.meta.size.w : 0, texH = d.meta.size != null ? d.meta.size.h : 0;
            if (PngSize(pngPath, out var w, out var h)) (texW, texH) = (w, h);
            var slices = texW > 0 && texH > 0 ? Slices(d, Path.GetFileNameWithoutExtension(pngPath), texW, texH, out _) : new SpriteMetaData[0];
            if (slices.Length == 0)
            {
                ti.spriteImportMode = SpriteImportMode.Single;
                return;
            }
            ti.spriteImportMode = SpriteImportMode.Multiple;
#pragma warning disable CS0618 // spritesheet: the 2D Sprite package's data provider would add a package dependency.
            ti.spritesheet = slices;
#pragma warning restore CS0618
        }

        static TextureImporterCompression Compression(string s)
        {
            switch ((s ?? "").ToLowerInvariant())
            {
                case "compressed": return TextureImporterCompression.Compressed;
                case "compressedhq": return TextureImporterCompression.CompressedHQ;
                case "compressedlq": return TextureImporterCompression.CompressedLQ;
                default: return TextureImporterCompression.Uncompressed;
            }
        }

        /// The frames a tag plays, in order. Ping-pong does not repeat its ends: 0, 1, 2, 1.
        internal static List<int> Sequence(int from, int to, string direction)
        {
            var seq = new List<int>();
            switch ((direction ?? "").ToLowerInvariant())
            {
                case "reverse":
                    for (int i = to; i >= from; i--) seq.Add(i);
                    break;
                case "pingpong":
                    for (int i = from; i <= to; i++) seq.Add(i);
                    for (int i = to - 1; i > from; i--) seq.Add(i);
                    break;
                default:
                    for (int i = from; i <= to; i++) seq.Add(i);
                    break;
            }
            return seq;
        }

        /// One clip's recipe: the frames it plays, whether it loops.
        class ClipPlan
        {
            public string name;
            public List<int> frames;
            public bool loop;
        }

        static List<ClipPlan> Plans(Data d)
        {
            var plans = new List<ClipPlan>();
            int n = d.frames.Length;
            if (n == 0) return plans;
            var tags = d.meta.frameTags ?? new Tag[0];
            var used = new HashSet<string>();
            for (int i = 0; i < tags.Length; i++)
            {
                var t = tags[i];
                if (t == null) continue;
                int from = Mathf.Clamp(Math.Min(t.from, t.to), 0, n - 1), to = Mathf.Clamp(Math.Max(t.from, t.to), 0, n - 1);
                var name = FileSafe(string.IsNullOrWhiteSpace(t.name) ? "tag" + i : t.name.Trim());
                for (int k = 2; !used.Add(name); k++) name = FileSafe(t.name) + "_" + k;
                plans.Add(new ClipPlan { name = name, frames = Sequence(from, to, t.direction), loop = t.repeat == 0 });
            }
            if (plans.Count == 0) plans.Add(new ClipPlan { name = "all", frames = Sequence(0, n - 1, "forward"), loop = true });
            return plans;
        }

        static string FileSafe(string s)
        {
            var sb = new StringBuilder();
            var bad = Path.GetInvalidFileNameChars();
            foreach (var c in (s ?? "").Trim())
                sb.Append(Array.IndexOf(bad, c) >= 0 || c == '/' || c == '\\' ? '_' : c);
            return sb.Length == 0 ? "tag" : sb.ToString();
        }

        /// The label every clip of one sheet carries, so an import can tell its clips from yours.
        static string LabelOf(string baseName)
        {
            var sb = new StringBuilder("GrayboxSheet_");
            foreach (var c in baseName) sb.Append(char.IsLetterOrDigit(c) || c == '_' || c == '-' ? c : '_');
            return sb.ToString();
        }

        /// Make (or update, in place) the clips and the controller of the sheet imported at `pngPath`,
        /// next to it. `target` is the component that shows the sprite: SpriteRenderer or Image. The
        /// controller is returned (null when there is nothing to play).
        internal static RuntimeAnimatorController BuildAnimation(Data d, string pngPath, Type target)
        {
            var dir = Path.GetDirectoryName(pngPath).Replace('\\', '/');
            var baseName = Path.GetFileNameWithoutExtension(pngPath);
            var label = LabelOf(baseName);
            if (!PngSize(pngPath, out var texW, out var texH))
            {
                texW = d.meta.size != null ? d.meta.size.w : 0;
                texH = d.meta.size != null ? d.meta.size.h : 0;
            }
            Slices(d, baseName, texW, texH, out var nameOfFrame);
            var sprites = new Dictionary<string, Sprite>();
            foreach (var o in AssetDatabase.LoadAllAssetRepresentationsAtPath(pngPath))
                if (o is Sprite s) sprites[s.name] = s;

            var clips = new List<(string tag, AnimationClip clip)>();
            var kept = new HashSet<string>();
            foreach (var plan in Plans(d))
            {
                var keys = new List<ObjectReferenceKeyframe>();
                float t = 0f;
                foreach (var i in plan.frames)
                {
                    if (!nameOfFrame.TryGetValue(i, out var sn) || !sprites.TryGetValue(sn, out var sprite)) continue;
                    keys.Add(new ObjectReferenceKeyframe { time = t, value = sprite });
                    var ms = d.frames[i] != null && d.frames[i].duration > 0 ? d.frames[i].duration : 100;
                    t += ms / 1000f;
                }
                if (keys.Count == 0) continue;
                // A clip ends at its last key (a stop time is not honoured), so the last picture gets a
                // closing key: it stays for its own duration, and a loop starts again after that.
                keys.Add(new ObjectReferenceKeyframe { time = t, value = keys[keys.Count - 1].value });
                var path = $"{dir}/{baseName}_{plan.name}.anim";
                clips.Add((plan.name, WriteClip(path, label, target, keys.ToArray(), t, plan.loop)));
                kept.Add(path);
            }
            if (clips.Count == 0) return null;
            var controller = WriteController(d, $"{dir}/{baseName}.controller", label, clips);
            // Clips of tags that are gone.
            foreach (var guid in AssetDatabase.FindAssets($"l:{label} t:AnimationClip", new[] { dir }))
            {
                var old = AssetDatabase.GUIDToAssetPath(guid);
                if (!kept.Contains(old) && Path.GetFileName(old).StartsWith(baseName + "_", StringComparison.Ordinal)) AssetDatabase.DeleteAsset(old);
            }
            AssetDatabase.SaveAssets();
            return controller;
        }

        static AnimationClip WriteClip(string path, string label, Type target, ObjectReferenceKeyframe[] keys, float length, bool loop)
        {
            var clip = AssetDatabase.LoadAssetAtPath<AnimationClip>(path);
            bool isNew = clip == null;
            if (isNew) clip = new AnimationClip { name = Path.GetFileNameWithoutExtension(path), frameRate = 60f };
            var binding = new EditorCurveBinding { path = "", type = target, propertyName = "m_Sprite" };
            AnimationUtility.SetObjectReferenceCurve(clip, binding, keys);
            var settings = AnimationUtility.GetAnimationClipSettings(clip);
            settings.loopTime = loop;
            settings.startTime = 0f;
            settings.stopTime = length;
            AnimationUtility.SetAnimationClipSettings(clip, settings);
            if (isNew) AssetDatabase.CreateAsset(clip, path);
            else EditorUtility.SetDirty(clip);
            AssetDatabase.SetLabels(clip, new[] { label });
            return clip;
        }

        /// One state per clip, named after its tag; the default one plays `meta.unity.play` (else the first).
        static AnimatorController WriteController(Data d, string path, string label, List<(string tag, AnimationClip clip)> clips)
        {
            var controller = AssetDatabase.LoadAssetAtPath<AnimatorController>(path);
            if (controller == null) controller = AnimatorController.CreateAnimatorControllerAtPath(path);
            if (controller.layers.Length == 0) controller.AddLayer("Base Layer");
            var machine = controller.layers[0].stateMachine;
            // States of clips that are gone (not ones you added).
            foreach (var cs in machine.states)
            {
                var old = cs.state.motion as AnimationClip;
                if (old == null || Array.IndexOf(AssetDatabase.GetLabels(old), label) < 0) continue;
                bool kept = false;
                foreach (var (_, clip) in clips) kept |= clip == old;
                if (!kept) machine.RemoveState(cs.state);
            }
            AnimatorState first = null, play = null;
            var want = d.meta.unity != null && !string.IsNullOrEmpty(d.meta.unity.play) ? FileSafe(d.meta.unity.play) : null;
            foreach (var (tag, clip) in clips)
            {
                AnimatorState state = null;
                foreach (var cs in machine.states)
                    if (cs.state.name == tag) state = cs.state;
                if (state == null) state = machine.AddState(tag);
                state.motion = clip;
                if (first == null) first = state;
                if (want != null && play == null && tag == want) play = state;
            }
            // (Not `??`: states are Unity objects.)
            machine.defaultState = play != null ? play : first;
            EditorUtility.SetDirty(controller);
            return controller;
        }
    }

    /// Sets up the sprite sheets Graybox exports into the project (`pixel.export`, format "unity"):
    /// a PNG with a Graybox JSON of the same name next to it.
    sealed class GrayboxSheetPostprocessor : AssetPostprocessor
    {
        void OnPreprocessTexture()
        {
            try
            {
                if (!assetPath.EndsWith(".png", StringComparison.OrdinalIgnoreCase)) return;
                var d = GrayboxSheet.ForTexture(assetPath);
                if (d == null) return;
                if (assetImporter is TextureImporter ti) GrayboxSheet.Configure(ti, d, assetPath, null);
            }
            catch (Exception e)
            {
                Debug.LogWarning($"Graybox: could not set up {assetPath} ({e.Message})");
            }
        }

        static void OnPostprocessAllAssets(string[] imported, string[] deleted, string[] moved, string[] movedFrom)
        {
            var done = new HashSet<string>(imported);
            var again = new List<string>();
            foreach (var path in imported)
            {
                try
                {
                    if (path.EndsWith(".png", StringComparison.OrdinalIgnoreCase))
                    {
                        var d = GrayboxSheet.ForTexture(path);
                        if (d != null) GrayboxSheet.BuildAnimation(d, path, typeof(SpriteRenderer));
                    }
                    else if (path.EndsWith(".json", StringComparison.OrdinalIgnoreCase))
                    {
                        // A new JSON, the PNG as it was: import the PNG again so it follows the JSON.
                        var png = Path.ChangeExtension(path, ".png");
                        if (!done.Contains(png) && File.Exists(png) && GrayboxSheet.ForTexture(png) != null) again.Add(png);
                    }
                }
                catch (Exception e)
                {
                    Debug.LogWarning($"Graybox: could not set up the animation of {path} ({e.Message})");
                }
            }
            foreach (var png in again) AssetDatabase.ImportAsset(png, ImportAssetOptions.ForceUpdate);
        }
    }
}
