use asr::{async_main, Address, game_engine::unity::{mono::Module, SceneManager, get_scene_name}, Error, future::{retry, next_tick}, PointerSize, Process, settings::Gui, string::{ArrayString}, timer, timer::TimerState, watcher::Watcher};
use bytemuck::CheckedBitPattern;
use derive;

static PROCESS_NAME : &str = "Risk of Rain 2.exe";

#[derive(Gui)]
struct AutoSplitterSettings {
    /// Allow the autosplitter to start the timer automatically
    #[default = true]
    start: bool,
    /// Allow the autosplitter to split automatically
    #[default = true]
    split: bool,
    /// Allow the autosplitter to reset automatically
    #[default = true]
    reset: bool,
}

#[derive(Gui)]
struct GameSettings {
    /// Split when leaving Bazaar Between Time
    #[default = true]
    bazaar: bool,
    /// Split when leaving Void Fields
    #[default = false]
    arena: bool,
    /// Split when leaving Gilded Shores
    #[default = false]
    goldshores: bool,
    /// Split when leaving Bulwark's Ambry
    #[default = false]
    artifactworld: bool,
    /// Don't split on stage transitions
    /// This excludes selected hidden realms and game end conditions
    #[default = false]
    fin: bool,
}

/// game state watchers
#[derive(Default)]
struct GameVars {
    fade: Watcher<f32>,
    stage_count: Watcher<i32>,
    results: Watcher<bool>,
    scene: Watcher<ArrayString<16>>,
}

/// state for update loop
#[derive(Default)]
struct AutoSplitterState {
    was_loading: bool,
}

/// MonoClass companion
struct StaticField<'a> {
    process: &'a Process,
    base_address: Address,
    field_offset: u64
}

impl StaticField<'_> {
    fn read_value<T: CheckedBitPattern>(&self) -> Result<T, Error> {
        return self.process.read_pointer_path::<T>(self.base_address, PointerSize::Bit64, &[0, self.field_offset]);
    }
}

/// Auto splitter logic update loop
fn update_loop(game_state: &GameVars, game_settings: &GameSettings, autosplitter_state: &mut AutoSplitterState, autosplitter_settings: &AutoSplitterSettings) {
    match timer::state() {
        TimerState::NotRunning => {
            if should_start(&game_state) && autosplitter_settings.start {
                timer::start();
            }
        },

        TimerState::Running | TimerState::Paused => {
            if should_reset(&game_state) && autosplitter_settings.reset {
                timer::reset();
            }
            if should_split(&game_state, game_settings) && autosplitter_settings.split {
                timer::split();
            }
            if is_loading(&game_state, autosplitter_state.was_loading) {
                if !autosplitter_state.was_loading {
                    timer::pause_game_time();
                    autosplitter_state.was_loading = true;
                }
            } else {
                if autosplitter_state.was_loading {
                    timer::resume_game_time();
                    autosplitter_state.was_loading = false;
                }
            }
        },

        TimerState::Ended | TimerState::Unknown => (),

        _ => todo!()
    }
}

/// Start on regular Stage 1s during fade-in
fn should_start(game_state: &GameVars) -> bool {
    if let (Some(scene), Some(fade)) = (game_state.scene.pair, game_state.fade.pair) {
        if scene.current.starts_with("golemplains") ||
           scene.current.starts_with("blackbeach") ||
           scene.current.starts_with("snowyforest") ||
           scene.current.starts_with("lakes") ||
           scene.current.starts_with("village")
        {
            return fade.current < 1.0 && fade.old >= 1.0;
        }
    }
    return false;
}

/// Reset on certain menu screens
fn should_reset(game_state: &GameVars) -> bool {
    if let Some(scene) = game_state.scene.pair {
        return match scene.current.as_str() {
            "lobby" | "title" | "crystalworld" | "eclipseworld" | "infinitetowerworld"
                => true,
            _ => false
        }
    }
    return false;
}

/// Split on stage increment, special scenes, and game end conditions
fn should_split(game_state: &GameVars, settings: &GameSettings) -> bool {
    // stage count increased
    if let Some(stage_count) = game_state.stage_count.pair {
        if !settings.fin && stage_count.current >= 1 && stage_count.increased() {
            return true;
        }
    }
    if let Some(scene) = game_state.scene.pair {
        // reached a special scene
        if scene.changed() {
            match scene.old.as_str() {
                "outro" => return true,
                "bazaar" => return settings.bazaar,
                "arena" => return settings.arena,
                "goldshores" => return settings.goldshores,
                "artifactworld" => return settings.artifactworld,
                _ => ()
            }
        }
        // completed a run on specific scenes
        if let Some(results) = game_state.results.pair {
            if results.changed_to(&true) {
                match scene.current.as_str() {
                    "limbo" | "mysteryspace" | "voidraid" => return true,
                    _ => ()
                }
            }
        }
    }
    return false;
}

/// Game is loading when FadeToBlackManager.alpha is increasing from 0->2.0 or at 2.0
fn is_loading(game_state: &GameVars, unchanged_state: bool) -> bool {
    if let Some(fade) = game_state.fade.pair {
        if fade.increased() {
            return true;
        }
        if fade.decreased() && fade.current > 0.0 || fade.current == 0.0 {
            return false;
        }
    }
    // maintain previous state when fade in/out is undetermined (aka current == previous)
    return unchanged_state;
}

async_main!(stable);

async fn main() {
    let autosplitter_settings = AutoSplitterSettings::register();
    let game_settings = GameSettings::register();
    let process_name = { if asr::get_os().ok().unwrap().starts_with("linux") {&PROCESS_NAME[0..15]} else {PROCESS_NAME} };

    loop {
        let process = Process::wait_attach(process_name).await;
        process.until_closes(async {
            //let monomod = Module::wait_attach(&process, asr::game_engine::unity::mono::Version::V3).await;
            let monomod = Module::wait_attach_auto_detect(&process).await;
            let sceneman = SceneManager::wait_attach(&process).await;

            // Workaround for version detection: wait until the scene is valid
            // before attempting to load RoR2.dll/Assembly-CSharp.dll
            // FIXME replace with file check + wait_get_image
            retry(|| sceneman.get_current_scene_path::<256>(&process)).await;

            // SotV onwards uses RoR2.dll, earlier versions use Assembly-CSharp.dll
            // FIXME breaks version assumption if RoR2.dll has not yet loaded
            // check if file "RoR2.dll" exists once wasi support is merged.
            if let Some(ror2) = monomod.get_image(&process, "RoR2").or(monomod.get_default_image(&process)) {

                // FadeToBlackManager exists almost at the start of the process, but starts off invalid
                let mut ftbm = ror2.get_class(&process, &monomod, "FadeToBlackManager");
                // Run exists from entering the lobby onwards
                let mut run = ror2.get_class(&process, &monomod, "Run");
                // GameOverController exists just before the end of a run, including dying
                let mut goc = ror2.get_class(&process, &monomod, "GameOverController");
                // alpha valid when FadeToBlackManager exists
                let mut alpha_loc : Option<Address> = None;
                // stageClearCount only valid during a run (not valid in the lobby)
                let mut stage_loc : Option<StaticField> = None;
                // shouldDisplayGameEndReportPanels valid when GameOverController exists
                let mut panel_loc : Option<StaticField> = None;

                let mut state = GameVars::default();
                let mut autosplitter_state = AutoSplitterState::default();

                loop {
                    // attmept to reload class fields when invalid
                    if ftbm.is_none() {
                        ftbm = ror2.get_class(&process, &monomod, "FadeToBlackManager");
                        alpha_loc = None;
                    }

                    if run.is_none() {
                        run = ror2.get_class(&process, &monomod, "Run");
                        stage_loc = None;
                    }

                    if goc.is_none() {
                        goc = ror2.get_class(&process, &monomod, "GameOverController");
                        panel_loc = None;
                    }

                    if let Some(ftbm) = ftbm.as_ref() {
                        if alpha_loc.is_none() {
                            let alpha_offset = ftbm.get_field_offset(&process, &monomod, "alpha");
                            let alpha_addr = ftbm.get_static_table(&process, &monomod);
                            if let (Some(alpha_offset), Some(alpha_addr)) = (alpha_offset, alpha_addr) {
                                alpha_loc = Some(alpha_addr.add(alpha_offset.into()));
                            }
                        }
                    }

                    if let Some(run) = run.as_ref() {
                        if stage_loc.is_none() {
                            let instance_field = run.get_field_offset(&process, &monomod, "<instance>k__BackingField");
                            let scc_field = run.get_field_offset(&process, &monomod, "stageClearCount");
                            let static_table = run.get_static_table(&process, &monomod);
                            if let (Some(instance_field), Some(static_table), Some(scc_field)) = (instance_field, static_table, scc_field) {
                                let instance_addr = static_table.add(instance_field.into());
                                stage_loc = Some(StaticField{process: &process, base_address: instance_addr, field_offset: scc_field.into()})
                            }
                        }
                    }

                    if let Some(goc) = goc.as_ref() {
                        if panel_loc.is_none() {
                            let instance_field = goc.get_field_offset(&process, &monomod, "<instance>k__BackingField");
                            let sdgerp_field = goc.get_field_offset(&process, &monomod, "<shouldDisplayGameEndReportPanels>k__BackingField")
                                .or_else(|| goc.get_field_offset(&process, &monomod, "_shouldDisplayGameEndReportPanels")); // versions after SotS (starting with manifest 4567638355138669926 on 2024-08-27)
                            let static_table = goc.get_static_table(&process, &monomod);
                            if let (Some(instance_field), Some(static_table), Some(sdgerp_field)) = (instance_field, static_table, sdgerp_field) {
                                let instance_addr = static_table.add(instance_field.into());
                                panel_loc = Some(StaticField{process: &process, base_address: instance_addr, field_offset: sdgerp_field.into()})
                            }
                        }
                    }

                    // update game state watchers
                    // make old = current when updating from an invalid state
                    if alpha_loc.is_some() {
                        if state.fade.pair.is_none() {
                            state.fade.update( process.read::<f32>(alpha_loc.unwrap()).ok() );
                        }
                        state.fade.update( process.read::<f32>(alpha_loc.unwrap()).ok() );
                    } else {
                        state.fade.update(None);
                    }

                    if let Some(stage_loc) = stage_loc.as_ref() {
                        if state.stage_count.pair.is_none() {
                            state.stage_count.update( stage_loc.read_value::<i32>().ok() );
                        }
                        state.stage_count.update( stage_loc.read_value::<i32>().ok() );
                    } else {
                        state.stage_count.update(None);
                    }

                    if let Some(panel_loc) = panel_loc.as_ref() {
                        if state.results.pair.is_none() {
                            state.results.update( panel_loc.read_value::<bool>().ok() );
                        }
                        state.results.update( panel_loc.read_value::<bool>().ok() );
                    } else {
                        state.results.update(None);
                    }

                    // update the scene name
                    // skip scene name updates during scene transitions (always invalid)
                    if let Some(scene) = sceneman.get_current_scene_path::<256>(&process).ok() {
                        let utf8_scene = std::str::from_utf8(get_scene_name(scene.as_bytes())).unwrap_or_default();
                        state.scene.update(ArrayString::<16>::from(&utf8_scene).ok());
                    }


                    // show state for debugging
                    match state.fade.pair {
                        Some(fade) => timer::set_variable("alpha", &format!("{0:?}", fade.current)),
                        _ => timer::set_variable("alpha", "[invalid]")
                    }
                    match state.stage_count.pair {
                        Some(stage_count) => timer::set_variable("stageClearCount", &format!("{0:?}", stage_count.current)),
                        _ => timer::set_variable("stageClearCount", "[Invalid]")
                    }
                    match state.results.pair {
                        Some(results) => timer::set_variable("shouldDisplayGameEndReportPanels", &format!("{0:?}", results.current)),
                        _ => timer::set_variable("shouldDisplayGameEndReportPanels", "[invalid]")
                    }
                    match state.scene.pair {
                        Some(scene) => timer::set_variable("scene name", &format!("{0:?}", scene.current)),
                        _ => timer::set_variable("scene name", "[invalid]")
                    }

                    // enter the autosplitter logic loop with updated state
                    update_loop(&state, &game_settings, &mut autosplitter_state, &autosplitter_settings);
                    next_tick().await;
                }
            }
        }).await;
    }
}

