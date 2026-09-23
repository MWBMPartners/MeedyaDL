// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Remembering a single answer the person gave outside the Settings screen.
//!
//! # Why this file exists
//!
//! The app asks a handful of one-off questions away from the Settings
//! screen — did you finish setting up, may we send crash reports, don't
//! ask me again before aborting the queue, shut the computer down when
//! this queue finishes, here is where my copy of FFmpeg lives.
//!
//! Every one of those was written into the page's own copy of the
//! settings and never to disk. Only two buttons in the entire app write
//! settings to disk: "Save Changes" on the Settings screen, and
//! "Re-run Setup Wizard" beside it. Nothing else did.
//!
//! So eight finished features did nothing at all:
//!
//! * Finishing the setup wizard was never recorded, so the app never
//!   knew setup had been done. That also meant the crash-reporting
//!   question, which only appears once setup is recorded as finished,
//!   **could never be asked on any install**.
//! * Answering the crash-reporting question was forgotten either way.
//! * "Not now" on the macOS "move me to Applications" prompt was
//!   forgotten, so it came back at every launch, for ever.
//! * "Don't ask again" before aborting the queue was forgotten.
//! * "Shut the computer down when the queue finishes" was forgotten —
//!   and the status bar said it was armed. Someone could set it, walk
//!   away, and come back to a machine still running.
//! * Pointing the setup wizard at a copy of FFmpeg, mp4decrypt, MP4Box,
//!   MediaInfo or N_m3u8DL-RE you already had was thrown away, so the
//!   app carried on as though the program were missing.
//! * Choosing a cookies file by hand in the setup wizard was thrown
//!   away. (The other two ways of supplying cookies were fine — the
//!   backend writes those itself.)
//!
//! Two separate reviewers, reading different parts of the app with no
//! knowledge of each other's work, found this independently.
//!
//! # Why it does not simply save the settings
//!
//! Because sending the whole settings object is the bug that was fixed
//! twice already (#1175) and backed out twice. The page's copy may hold
//! half-finished edits from the Settings screen that nobody has pressed
//! Save on. Writing all of it would commit edits the person never chose
//! to commit.
//!
//! So this takes ONE named answer at a time. [`StoredPreference`] is a
//! closed list: every variant names exactly one thing and carries only
//! the value that belongs to it. **There is no variant a settings object
//! could arrive through**, which is the whole point — the same property
//! `set_sidebar_collapsed` was built for, applied to the rest of the
//! questions instead of only that one.
//!
//! The write itself goes through `config_service::update_settings_field`,
//! which reads the file, changes the one field, and writes it back while
//! holding the same lock "Save Changes" takes.

use tauri::AppHandle;

use crate::models::settings::AfterQueueAction;
use crate::services::config_service;

/// Which of the five helper programs a remembered path belongs to.
///
/// A closed list rather than a field name as text: a name arriving as
/// text could be anything, including a field that is not a program path
/// at all. This way only these five can be named, and the compiler
/// checks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HelperProgram {
    Ffmpeg,
    Mp4Decrypt,
    Mp4Box,
    Nm3u8DlRe,
    MediaInfo,
}

/// One answer, given outside the Settings screen, that should be
/// remembered.
///
/// Adding a variant here is how a new one-off question gets remembered.
/// Do not add a variant that carries several unrelated fields, and never
/// add one that carries a whole settings object — see the file notes.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StoredPreference {
    /// The setup wizard was finished.
    SetupCompleted { completed: bool },

    /// The crash-reporting question was answered.
    ///
    /// One variant for both fields on purpose: the answer and the fact
    /// that it was asked are a single event. Writing them separately
    /// would leave a gap in which the app has been told "yes" but does
    /// not know it asked, and would ask again.
    CrashReportingChoice { enabled: bool },

    /// "Not now" on the macOS offer to move the app into Applications.
    RelocationDeclined { declined: bool },

    /// Whether to ask before aborting everything in the queue.
    ///
    /// `false` means "do not ask" — the person ticked "don't ask again".
    AbortQueueConfirm { confirm: bool },

    /// What to do once, when the queue next finishes.
    ///
    /// `None` clears it. This is deliberately a one-off: it is cleared
    /// after it runs, which is why it is not on the Settings screen.
    AfterQueueOnce { action: Option<AfterQueueAction> },

    /// Where the person's own copy of a helper program lives.
    ///
    /// `None` clears it, which puts the app back to using the copy it
    /// manages itself.
    HelperProgramPath {
        program: HelperProgram,
        path: Option<String>,
    },

    /// Where the person's exported cookies file lives.
    CookiesPath { path: Option<String> },
}

/// Remember one answer given outside the Settings screen.
///
/// # Errors
///
/// Returns `Err(String)` if the settings file cannot be read or written.
/// The caller should say so rather than carrying on as though the answer
/// had been remembered — that silence is what this whole file exists to
/// end.
#[tauri::command]
pub async fn set_stored_preference(
    app: AppHandle,
    preference: StoredPreference,
) -> Result<(), String> {
    config_service::update_settings_field(&app, |s| apply(s, &preference))?;
    Ok(())
}

/// Put one answer into a settings object.
///
/// Split out from the command so the tests below call the real thing.
/// A test that wrote out the same matching a second time would pass
/// whatever this did, including nothing at all — which is precisely the
/// fault being fixed here, so it would be a poor way to fix it.
fn apply(s: &mut crate::models::settings::AppSettings, preference: &StoredPreference) {
    match preference {
        StoredPreference::SetupCompleted { completed } => {
            s.setup_completed = *completed;
        }
        StoredPreference::CrashReportingChoice { enabled } => {
            s.sentry_enabled = *enabled;
            // Recorded together with the answer, never separately — see
            // the variant's own note.
            s.crash_report_prompt_shown = true;
        }
        StoredPreference::RelocationDeclined { declined } => {
            s.relocation_declined = *declined;
        }
        StoredPreference::AbortQueueConfirm { confirm } => {
            s.abort_queue_confirm = *confirm;
        }
        StoredPreference::AfterQueueOnce { action } => {
            s.after_queue_once = *action;
        }
        StoredPreference::HelperProgramPath { program, path } => {
            let field = match program {
                HelperProgram::Ffmpeg => &mut s.ffmpeg_path,
                HelperProgram::Mp4Decrypt => &mut s.mp4decrypt_path,
                HelperProgram::Mp4Box => &mut s.mp4box_path,
                HelperProgram::Nm3u8DlRe => &mut s.nm3u8dlre_path,
                HelperProgram::MediaInfo => &mut s.mediainfo_path,
            };
            *field = path.clone();
        }
        StoredPreference::CookiesPath { path } => {
            s.cookies_path = path.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::AppSettings;

    #[test]
    fn finishing_setup_is_recorded() {
        let mut s = AppSettings::default();
        assert!(!s.setup_completed, "a fresh install has not been set up");

        apply(
            &mut s,
            &StoredPreference::SetupCompleted { completed: true },
        );
        assert!(s.setup_completed);
    }

    #[test]
    fn the_crash_reporting_answer_and_the_asking_are_recorded_together() {
        // The reason they are one variant: if the answer were written
        // without the "we asked" flag, the app would have permission it
        // does not know it has, and would ask again at the next launch.
        for answer in [true, false] {
            let mut s = AppSettings::default();
            apply(
                &mut s,
                &StoredPreference::CrashReportingChoice { enabled: answer },
            );
            assert_eq!(s.sentry_enabled, answer);
            assert!(
                s.crash_report_prompt_shown,
                "saying no still counts as having been asked"
            );
        }
    }

    #[test]
    fn each_helper_program_lands_in_its_own_field_and_no_other() {
        // The fault worth guarding against here is a copy-and-paste slip
        // that writes every program's path into the same field — which
        // would look right for whichever one was tested and be wrong for
        // the rest. So each is checked to change one field and leave the
        // other four alone.
        let cases = [
            (HelperProgram::Ffmpeg, "/usr/local/bin/ffmpeg"),
            (HelperProgram::Mp4Decrypt, "/usr/local/bin/mp4decrypt"),
            (HelperProgram::Mp4Box, "/usr/local/bin/MP4Box"),
            (HelperProgram::Nm3u8DlRe, "/usr/local/bin/N_m3u8DL-RE"),
            (HelperProgram::MediaInfo, "/usr/local/bin/mediainfo"),
        ];

        for (program, path) in cases {
            let mut s = AppSettings::default();
            apply(
                &mut s,
                &StoredPreference::HelperProgramPath {
                    program,
                    path: Some(path.to_string()),
                },
            );

            let landed = [
                s.ffmpeg_path.as_deref(),
                s.mp4decrypt_path.as_deref(),
                s.mp4box_path.as_deref(),
                s.nm3u8dlre_path.as_deref(),
                s.mediainfo_path.as_deref(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

            assert_eq!(
                landed,
                vec![path],
                "{program:?} must set exactly one path, and it must be its own"
            );
        }
    }

    #[test]
    fn a_helper_program_path_can_be_cleared() {
        let mut s = AppSettings {
            ffmpeg_path: Some("/somewhere/old".to_string()),
            ..AppSettings::default()
        };

        apply(
            &mut s,
            &StoredPreference::HelperProgramPath {
                program: HelperProgram::Ffmpeg,
                path: None,
            },
        );
        assert_eq!(
            s.ffmpeg_path, None,
            "clearing puts the app back on the copy it manages itself"
        );
    }

    #[test]
    fn the_one_off_after_queue_action_can_be_set_and_cleared() {
        let mut s = AppSettings::default();

        apply(
            &mut s,
            &StoredPreference::AfterQueueOnce {
                action: Some(AfterQueueAction::ShutdownComputer),
            },
        );
        assert_eq!(s.after_queue_once, Some(AfterQueueAction::ShutdownComputer));

        apply(&mut s, &StoredPreference::AfterQueueOnce { action: None });
        assert_eq!(s.after_queue_once, None);
    }

    #[test]
    fn dont_ask_again_before_aborting_is_recorded() {
        let mut s = AppSettings::default();
        apply(
            &mut s,
            &StoredPreference::AbortQueueConfirm { confirm: false },
        );
        assert!(!s.abort_queue_confirm);
    }

    #[test]
    fn declining_the_move_offer_is_recorded() {
        let mut s = AppSettings::default();
        apply(
            &mut s,
            &StoredPreference::RelocationDeclined { declined: true },
        );
        assert!(s.relocation_declined);
    }

    #[test]
    fn a_cookies_file_chosen_by_hand_is_recorded() {
        let mut s = AppSettings::default();
        apply(
            &mut s,
            &StoredPreference::CookiesPath {
                path: Some("/Users/someone/cookies.txt".to_string()),
            },
        );
        assert_eq!(
            s.cookies_path.as_deref(),
            Some("/Users/someone/cookies.txt")
        );
    }

    #[test]
    fn writing_one_answer_leaves_every_other_setting_alone() {
        // The property the whole file exists for. Sending the settings
        // object was the bug that got backed out twice; this checks that
        // one answer really does change one thing.
        let mut s = AppSettings {
            output_path: "/Users/someone/Music".to_string(),
            musickit_team_id: Some("TEAMID1234".to_string()),
            verbose_activity_log: true,
            ..AppSettings::default()
        };

        let before = s.clone();
        apply(
            &mut s,
            &StoredPreference::SetupCompleted { completed: true },
        );

        assert!(s.setup_completed);
        assert_eq!(s.output_path, before.output_path);
        assert_eq!(s.musickit_team_id, before.musickit_team_id);
        assert_eq!(s.verbose_activity_log, before.verbose_activity_log);
    }

    #[test]
    fn the_wire_shape_is_what_the_page_actually_sends() {
        // Checks the names the page uses, because a mismatch here fails
        // at runtime with "invalid args" and nothing in Rust would catch
        // it. Written as the JSON the page sends, not built from the
        // enum, so a rename on this side shows up as a failure rather
        // than quietly agreeing with itself.
        let parsed: StoredPreference =
            serde_json::from_str(r#"{"kind":"setup_completed","completed":true}"#).unwrap();
        assert_eq!(parsed, StoredPreference::SetupCompleted { completed: true });

        let parsed: StoredPreference = serde_json::from_str(
            r#"{"kind":"helper_program_path","program":"mp4_box","path":"/opt/homebrew/bin/MP4Box"}"#,
        )
        .unwrap();
        assert_eq!(
            parsed,
            StoredPreference::HelperProgramPath {
                program: HelperProgram::Mp4Box,
                path: Some("/opt/homebrew/bin/MP4Box".to_string()),
            }
        );

        let parsed: StoredPreference =
            serde_json::from_str(r#"{"kind":"after_queue_once","action":null}"#).unwrap();
        assert_eq!(parsed, StoredPreference::AfterQueueOnce { action: None });
    }
}
