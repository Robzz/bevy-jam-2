use bevy::prelude::*;
use iyes_loopless::{prelude::*, state::StateTransitionStageLabel};

mod level;
mod level_processor;

pub use level::Level;
pub use level_processor::{LevelProcessor, SceneAnimationPlayer};

use self::level_processor::ColliderShape;

use super::game::GameState;

#[derive(Debug, Default, PartialEq)]
enum SpawnState {
    #[default]
    Idle,
    Pending(Handle<Level>),
    ProcessingScene(Entity),
    Spawning,
    Finalizing,
}

pub struct LevelsPlugin;

impl Plugin for LevelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<Level>();
        app.register_type::<SceneAnimationPlayer>()
            .register_type::<ColliderShape>();
        app.insert_resource(LevelProcessor::new());

        app.add_enter_system(GameState::Loading, LevelProcessor::init_level_transition);
        app.add_exit_system(GameState::Loading, LevelProcessor::finalize_level_spawn);

        app.add_stage_after(
            StateTransitionStageLabel::from_type::<GameState>(),
            LevelManagerStages::SpawnLevel,
            SystemStage::single_threaded(),
        );
        app.add_system_to_stage(
            LevelManagerStages::SpawnLevel,
            LevelProcessor::spawn_level_system.run_in_state(GameState::Loading),
        );

        app.add_stage_after(
            LevelManagerStages::SpawnLevel,
            LevelManagerStages::PrepareScene,
            SystemStage::parallel(),
        );
        app.add_system_to_stage(
            LevelManagerStages::PrepareScene,
            LevelProcessor::postprocess_scene
                .run_in_state(GameState::Loading)
                .label(PrepareStageSystemLabels::ProcessScene),
        );
        app.add_system_to_stage(
            LevelManagerStages::PrepareScene,
            LevelProcessor::spawn_player
                .run_in_state(GameState::Loading)
                .label(PrepareStageSystemLabels::SpawnPlayer)
                .after(PrepareStageSystemLabels::ProcessScene),
        );
        app.add_system_to_stage(
            LevelManagerStages::PrepareScene,
            LevelProcessor::finalize_level_spawn
                .run_in_state(GameState::Loading)
                .label(PrepareStageSystemLabels::Finalize)
                .after(PrepareStageSystemLabels::SpawnPlayer),
        );

        app.add_system(LevelProcessor::gltf_asset_event_listener);
        app.add_system(LevelProcessor::check_level_loading_progress);
    }
}

#[derive(Debug, StageLabel)]
pub enum LevelManagerStages {
    SpawnLevel,
    PrepareScene,
}

#[derive(Debug, SystemLabel)]
enum PrepareStageSystemLabels {
    ProcessScene,
    SpawnPlayer,
    Finalize,
}
