//! The world's pieces for the renderer. PLACEHOLDER (G11.1): the simulation's flat ground and blocks,
//! built once into the static buffer. G11.2 replaces this with the valley's terrain, grass and props.
use figcraft_game::arena::SceneObject;
use figcraft_game::tps::World;

pub(in crate::game::renderer) fn objects() -> Vec<SceneObject> {
    World::new().objects()
}
