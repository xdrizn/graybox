# Low-poly test scene — spec

> **Read this first (Greybox adaptation, 2026-11-03).** This handoff was written for an earlier web
> prototype (three.js) and suggested Bevy. In Greybox it is the **visual and gameplay spec** for the
> Play mode scene; §1–§5 (look, tricks, arena layout, pistol, Sentinel) are to be followed closely.
> These parts are **overridden**:
>
> - **§6 Rust / Bevy: do not use Bevy.** FigCraft draws with eframe + egui-wgpu on **wgpu 30**; Bevy
>   pins its own wgpu, so the two cannot share a device or textures, it would double the wasm size
>   and bring a second app loop. Render the scene with **plain wgpu inside an egui paint callback**
>   (`crates/ui-egui/src/game/`), sharing egui's device: flat-shaded primitives (box, wedge, 6-sided
>   cylinder, octahedron), world-space/triplanar UVs with the 16×16 nearest noise textures, exp² fog,
>   one directional light with a shadow map, the orange point light, and a small threshold + blur
>   bloom pass for the accent only. Static geometry batched into one mesh per tone. Must work on
>   WebGPU and the WebGL2 fallback.
> - **Physics:** no engine. The arena is axis-aligned boxes plus one ramp: a small kinematic
>   character controller in `crates/game` (AABB/capsule vs boxes, step-up, slope, ground snap) is
>   enough; `parry3d` is acceptable if hand-rolled collision gets messy. Raycasts for shots and the
>   Sentinel's line of sight against the same static set.
> - **§7 UI: do not port.** In Greybox the UI is whatever the designer builds in FigCraft (frames,
>   components, variables, prototype interactions). §7 is only background for how the earlier
>   prototype behaved; reuse its **data and event names** as the defaults of the Game variables and
>   Game event trigger (G3): data `vida, vidaMax, municao, reserva, atacante, ultimoDano, arma`;
>   events `tomou-dano, vida-zero, tela-abriu`… (renamable).
> - **"Background mode"** (§6, dolly camera with no player) is a nice extra for the editor canvas
>   preview; Play mode is the playable FPS.
> - Reference images mentioned below live in `docs/game/references/` (add them with rows in
>   `ATTRIBUTION.md`; they are the owner's own work).
> - The editor's own look (black/orange, Space Grotesk / JetBrains Mono) comes from
>   [`tokens.json`](tokens.json), mapped onto FigCraft's `theme::Tokens` (see PLAN.md, G1).

---

# HANDOFF → Claude Code · cenário low poly de fundo em Rust

> Para agente. Denso de propósito. Origem: protótipo web (R3F/three.js) em `sandbox/`. Alvo: Rust, só o **cenário de fundo** (jogo atrás da UI), não a ferramenta inteira.
> Tokens/fonte da UI: `tokens.json` ao lado. Referências visuais: `../references/fps-visual-ref.png`, `../references/pistola-*.png`, `../references/indirect-brand-ref.png`.

## 0. TL;DR do look

`mono(preto→cinza) + laranja SÓ emissivo + flat shading + textura 16² nearest + fog exp² escura + 1 key light c/ sombra + bloom só no laranja`. Tudo é primitiva (caixa, cunha, cilindro de 6 lados). Nenhum modelo importado. Um tema troca o jogo inteiro.

## 1. Tema (fonte da verdade do cenário)

```
GRAYS   = [#101010, #262626, #3d3d3d, #5a5a5a, #7c7c7c, #a3a3a3]   // tone 0..5, preto→claro
ACCENT  = #ff5a00          // mesmo primário da marca INDIRECT
ACCENT_EMISSIVE = 1.6      // >1 p/ passar no bloom; >~2.5 satura p/ amarelo (evitar)
BG      = #030303
FOG     = exp², cor #060606, densidade 0.02
AMBIENT = 0.35 ; HEMI sky #bfbfbf ground #0d0d0d 0.7
KEY     = dir, #f2f2f2, 3.2, pos (14,34,18) → origem, sombra 2048², ortho ±35×±40, far 90, bias -0.0004
PORTAL  = point #ff5a00, 50, range 40, decay 1.6, em (0,3,-37)
BLOOM   = intensity 0.85, threshold 0.9, smoothing 0.2, mip blur
CAM     = fov 75°, near 0.05, far 200
```
Regra dura: **laranja nunca em superfície difusa**. Só em: portal, faixas de luz, frestas, janelinhas, estandarte, fresta do rosto do inimigo, projéteis, flash do tiro, foco de UI.

## 2. Truques que fazem o low poly ficar "bonito, não sujo"

1. **Flat shading**: normal por face. Bevy: `mesh.duplicate_vertices(); mesh.compute_flat_normals();` (cuboid já é por face; vale p/ cilindro 6 lados, dodecaedro, cunhas).
2. **Pixel grande**: textura 16×16 de ruído **por tom de cinza**, cada pixel = `base * U(0.82, 1.18)`, PRNG LCG fixo (`s = (s*9301+49297) % 233280`) c/ seed = tone+1 → determinístico. Filtro **nearest** (mag+min), wrap repeat, sRGB. Cor do material = branco (a textura carrega o cinza).
3. **Tamanho do pixel constante no mundo**: `uv_repeat = (max(sx, sz)/2, sy/2)`, ou seja, ~2 m por tile. Sem isso, caixas grandes ficam com pixel gigante. Melhor ainda no Rust: **UV triplanar/world-space no shader** (`uv = world.xz/2` etc.) e esquece repeat por mesh.
4. **Chão "molhado" barato**: chão tone 1 com roughness **0.35** (resto 0.9) + a point light laranja do portal → reflexo especular laranja borrado. Sem SSR.
5. **Bloom só pega o laranja**: emissive alto, threshold alto. Cinzas nunca passam de 1.0.
6. **Fog escura** some com teto e fundo → profundidade + esconde o pop-in.
7. Toggle `pixel_textures: bool` → sem textura, cor sólida do tom. Útil p/ comparar.

## 3. Layout da arena (metros, Y pra cima, jogador olha -Z)

`B(pos; size; tone; rot?)` = caixa centrada em pos. `G(...)` = glow (accent emissivo, sem colisão). `nc` = sem colisão.

```
chão      B(0,-0.5,-10; 40,1,62; t1, rough .35)
paredes   B(±20.5,9,-10; 1,18,62; t2)
          B(±11.05,9,-40.5; 17.9,18,1; t2)      // fundo com vão central de 4.2 p/ o portal
          B(0,13.25,-40.5; 4.2,9.5,1; t2)       // lintel acima do portal
          B(0,9,21.5; 42,18,1; t2)              // trás
teto      B(0,18.5,-10; 42,1,64; t0; nc)
pilares   x∈{-9,9} × z∈{6,-6,-18,-30}:
            fuste B(x,7,z; 2.4,14,2.4; t3) ; base B(x,.7,z; 3.4,1.4,3.4; t2) ; capitel B(x,13.4,z; 3,1.2,3; t2; nc)
            G(x∓1.71,.8,z; .02,.5,.18)          // fresta na base, face interna
escada    10 degraus, x=15, w=5, h=.4, d=1, de z=-2 p/ -Z: degrau i = B(15, .4(i+1)/2, -2-i-.5; 5,.4(i+1),1; t4)
patamar   B(15,2,-17; 10,4,10; t3) ; mureta B(11.2,4.5,-17; .4,1,10; t2)
rampa     B(-15,1.5,-2; 4,.4,10.4; t4; rotX = -atan2(3,10)) → plataforma B(-15,1.5,-12; 6,3,8; t3)
caixas    s=2 (ou 1.6): B(x,s/2,z; s³; t2) + X em frente/trás: 2× B(0,0,±(s/2+.01); 1.25s,.16,.04; t1; rotZ ±π/4; nc)
          em (-5,0,2) (-3,0,2) (5,0,-10) (-6,0,-22)s1.6 (3,0,-28) (16,4,-20)s1.6
entulho   40 cubinhos nc, t4, seed 7, lado .15–.6, altura ×.5–1, x∈±17, z∈[-35,15], rotY aleatória
portal    G(0,4,-40.6; 4.2,8.6,.1; e=2) + ombros do arco B(±1.9,7.6,-40; 2.4,1.6,1; t2; rotZ ∓π/4.5; nc) → ponta gótica
          + PORTAL light
estandarte em (-19.9,8,-14), rotY π/2: shape 1.6×4 c/ entalhe em V embaixo (pontos (-.8,2)(.8,2)(.8,-2)(0,-1.4)(-.8,-2)), accent e=0.9 ; haste B(0,2.05,.05; 2.2,.08,.08; t1)
faixa chão G(-6,.02,-26; .14,.02,30; rotY .42; e=1.8)   // linha laranja vindo do portal
janelinhas 3×3 G(19.95, 11+.8j, -26+.8i; .02,.45,.45; e=2)
frestas   G(-19.95,5,0; .02,2.2,.25)  G(19.95,4,8; .02,1.6,.2)  G(-4,12,-39.95; .3,1.6,.02)
spawn     (0,2,12), olhando -Z
```

## 4. Pistola (viewmodel) — eixos: -Z cano, +Y cima; 3 materiais

`slide = t2 rough .55` · `frame = t1 rough .7` · `black = t0 rough .6`. Coordenadas em metros (escala 1).

```
ferrolho trás   B(0,.031,.035; .062,.058,.13; slide)
ferrolho frente B(0,.035,-.115; .057,.05,.17; slide)          // degrau
rebaixo         B(0,.008,-.172; .059,.016,.055; frame)
janela ejeção   B(0,.056,-.06; .059,.012,.06; black)
ranhuras ×4     B(0,.032,z; .0635,.036,.005; black; rotX .38)  z∈{.032,.046,.06,.074}
cano            cilindro r.013 h.014 8 lados, rotX π/2, em (0,.03,-.204), black
mira frente     B(0,.066,-.188; .01,.013,.018; black; rotX -.3)
mira trás ×2    B(±.012,.066,.088; .011,.013,.012; black)
cão             B(0,.05,.106; .018,.024,.022; black; rotX -.45)
armação         B(0,-.012,-.04; .054,.028,.2; frame)
guarda-mato     B(0,-.06,-.046; .016,.008,.072; frame; rotX .08) + B(0,-.042,-.083; .016,.042,.009; frame; rotX .35)
gatilho         B(0,-.04,-.036; .008,.032,.009; black; rotX -.35)
beavertail      B(0,-.022,.084; .052,.03,.03; frame; rotX -.25)
cabo (grupo em (0,-.086,.054) rotX -.3):
                B(0,0,0; .056,.14,.07; frame)
                sulcos ×4 B(0,y,-.034; .06,.016,.016; black) y∈{.036,.009,-.018,-.045}
                base mag B(0,-.074,.003; .06,.012,.08; black)
flash           octaedro r.035, accent e=2.5, em (0,.03,-.23), visível 50 ms após tiro
```
Pose na câmera (espaço da câmera): `pos (0.2+bx, -0.19-by-0.18·dip, -0.44+0.05·kick)`, `rot (0.05+0.2·kick-0.9·dip, 0.04, -0.06+0.4·dip)`.
`kick = max(0, 1 - t_tiro/0.12)`, `dip = sin(π·t_recarga/0.8)` em [0,0.8 s], `bob`: fase += dt·11 andando (7 agachado); `bx = sin(fase)·.012`, `by = |cos(fase)|·.012`; câmera ainda sobe/desce `sin(2·fase)·.03`.
Yaw 0.04 = cano quase reto pra mira (o usuário pediu; 0.42 mostrava a lateral mas parecia apontar p/ esquerda).

## 5. Inimigo "Sentinela" (opcional no fundo)

Blocos t2/t3: pernas 2× B(±.25,.5,0; .38,1,.45), torso B(0,1.55,0; 1.15,1.1,.65), cabeça B(0,2.35,0; .7,.55,.6), fresta G(0,2.38,.305; .46,.07,.02).
Olha p/ jogador só no yaw. Ao levar tiro: torso/cabeça emissive accent 2.5 por 150 ms. 3 tiros → some e solta 9 cubos .35 (gravidade 18, quique ×0.4, chão y .17), volta em 5 s. Atira projétil octaedro r.16 accent e=2, 12 m/s, vida 4 s, se tiver linha de visão (raycast contra estáticos) até 24 m, a cada ~3.5 s ±20%.

## 6. Rust — recomendação

**Bevy (0.16+)**. API ≥0.15: `Mesh3d`, `MeshMaterial3d`, `DistanceFog`, `Bloom`, `Camera3d`. Verificar nomes na versão instalada; mudam entre minors.

| Peça web | Rust |
|---|---|
| three.js / R3F | `bevy` (PBR, sombras, bloom, fog prontos) |
| Rapier (KinematicCharacterController) | `avian3d` (nativo Bevy) ou `bevy_rapier3d` (mesmo `KinematicCharacterController`, autostep .45/.2, snap .4, slope 50°) |
| postprocessing Bloom | `Bloom { intensity: .15–.3, prefilter threshold ~0.9, .. }` + `Camera { hdr: true }` + `Tonemapping::TonyMcMapface` |
| FogExp2 | `DistanceFog { color, falloff: FogFalloff::ExponentialSquared { density: 0.02 } }` |
| CanvasTexture 16² | `Image::new(Extent3d{16,16,1}, D2, data, Rgba8UnormSrgb, ..)` + `image.sampler = ImageSampler::nearest()` + address mode Repeat |
| Gamepad API | `bevy::input::gamepad` (nativo) ou `gilrs` |
| UI DOM por cima | `bevy_ui` (Node/flex = auto-layout), ou `egui` p/ ferramenta |

Alternativas mais leves se Bevy for demais: `macroquad` (sem PBR; faz flat + fake light na mão) ou `three-d`. Para **só fundo**, Bevy ainda é o caminho mais curto por já ter bloom/fog/sombra.

### Esqueleto (Bevy, só fundo, sem física)

```rust
// tema
const GRAYS: [&str; 6] = ["#101010","#262626","#3d3d3d","#5a5a5a","#7c7c7c","#a3a3a3"];
const ACCENT: Color = Color::srgb(1.0, 90.0/255.0, 0.0);

fn noise_tex(hex: &str, seed: u32) -> Image {
    let base = Srgba::hex(hex).unwrap();
    let mut s = seed.wrapping_mul(9301).wrapping_add(49297);
    let mut px = Vec::with_capacity(16*16*4);
    for _ in 0..256 {
        s = (s.wrapping_mul(9301).wrapping_add(49297)) % 233280;
        let k = 0.82 + (s as f32 / 233280.0) * 0.36;
        px.extend([(base.red*k*255.) as u8, (base.green*k*255.) as u8, (base.blue*k*255.) as u8, 255]);
    }
    let mut img = Image::new(Extent3d{width:16,height:16,depth_or_array_layers:1},
        TextureDimension::D2, px, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat, address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::nearest() });
    img
}
// material cinza: base_color WHITE, base_color_texture = noise_tex(GRAYS[t], t+1), perceptual_roughness .9 (chão .35), metallic 0
// material accent: base_color ACCENT, emissive = LinearRgba::from(ACCENT) * 1.6 (ajustar: Bevy usa unidades físicas; testar 2–8)
// cuboid: Mesh::from(Cuboid::new(sx,sy,sz)) e escalar UV (ou shader triplanar) p/ ~2 m por tile
// cilindro 6 lados: Cylinder::new(r,h).mesh().resolution(6).build() + duplicate_vertices + compute_flat_normals
```

Câmera: `Camera3d` + `Camera{hdr:true}` + `Projection::Perspective{fov: 75°.to_radians(), near:.05, far:200}` + `DistanceFog` + `Bloom`. `ClearColor(BG)`. `AmbientLight{brightness ≈ calibrar}`; luz direcional c/ `shadows_enabled: true` e `CascadeShadowConfigBuilder` (ou ortho fixo ±40).

### Modo "fundo" (sugestões p/ ficar vivo sem jogador)

- **Dolly lento**: câmera em trilho z 12 → -20 a ~0.6 m/s, ping-pong, yaw senoidal ±8°, bob leve. Pistola opcional (sem ela vira "ambient").
- **Respiração de luz**: portal light `50 + 6·sin(t·0.7)`; faixas laranja emissive `1.6 ± 0.15`.
- **Poeira**: 200 quads aditivos minúsculos cinza claros subindo devagar dentro do cone do feixe (simula o raio de luz da referência).
- **Sentinelas em patrulha**: 2 inimigos andando entre 2 pontos, sem tiro, fresta piscando.
- **Parallax com a UI**: passar o offset do mouse/stick p/ yaw ±2° da câmera → fundo reage ao menu.
- **Desempenho**: mesclar estáticos num mesh por tom (batch), 1 sombra só (key), MSAA off + `Fxaa` (combina c/ pixel), alvo 60 fps em iGPU.

## 7. Como a UI foi feita (contexto, caso precise reproduzir)

- UI = **DOM sobre o canvas**, numa "folha" de referência 1920×1080 escalada por `s = min(W/1920, H/1080)` com folha lógica `W/s × H/s` (modo "Expand" da Unity → âncoras chegam na borda real em 21:9/32:9).
- Peça = `{type: frame|rect|image|text|stack-v|stack-h|grid, anchor(9 pts), x,y,w,h, style, bar?, text?, nav?, action?, reactions[]}`. `frame` = posição livre; stack/grid = flexbox/grid (= auto-layout Figma).
- **Wireframe INDIRECT**: borda 2px + vão transparente (padding) + miolo cheio (opcional listras `repeating-linear-gradient(to bottom, cor 0 2px, transparente 2px 5px)`). Barra = largura do miolo `valor/max`.
- Jogo ↔ UI só por **bus de eventos** (`tomou-dano`, `vida-zero`, `tela-abriu`…) e **dados** (`vida, vidaMax, municao, reserva, atacante, ultimoDano, arma`). Texto usa `{vida}` etc.
- Telas = pilha; persistentes desenhadas por cima; foco por direcional com `nextFocus` (sem wrap, pula ocultos); foco = borda laranja `#ff5a00` fixa (não segue tema da ferramenta).
- Reações = Web Animations API (fade/slide/scale/pulse/shake). Em Bevy: tween de `Transform`/`BackgroundColor` por evento (`bevy_tweening`).
- Ferramenta (editor) usa **tokens semânticos** em `tokens.css` → ver `tokens.json`. Componentes nunca usam hex direto.
