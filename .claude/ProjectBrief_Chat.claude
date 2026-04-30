Please create a fully multiplatform GUI application to be used with GAMDL (https://github.com/glomatico/gamdl)

The concept is that the application can provide a complete, but simple and user friendly way to use GAMDL.

The project/application must allow for use of ALL features offiered by GAMDL, including:

* music download (specific song, album, album artist, artist, composer, playlist, music videos)
* Download any the quality set in the settings/preferences but ALSO allow to override on a per download request basis (a download request is every time the USER queues a download).
* Download and embedding the highest quality cover art (including adhering to the cover format selected in settings/preferences - if unset choose "RAW")
* Download and embedding of lyrics
    - For music download and embed the LRC format file
    - For music videos download ttml closed captions/subtitles and embed as soft captions
        - Alternatively download the LRC file, convert to TTML and embed)
    - Download SRT and place in same folder, named identically to song file name.
* Add queueing of downloads (if possible)
* Add fallback download quality architecture (if possible)
    ie. Allow a user to set downlaod priority preferences, with fall back to the next priority should one fail. Such as (with this serving as the fault quality if none is very specifically defined in settings/preferences):
        MUSIC
            - Lossless (ALAC)
            - Dolby Atmos
            - Dolby Digital (AC3)
            - AAC (256kbps) Binaural
            - AAC (256kbps at up to 48kHz)
            - AAC Legacy (256kbps at up to 44.1 kHZ)

        MUSIC VIDEOS
            - H.265 2160p
            - H.265 1440p
            - H.265 1080p
            - H.264 1080p
            - H.264 720p
            - H.264 540p
            - H.264 480p
            - H.264 360p
            - H.264 240p


Additional features such as settings management, should also be supported within the UI, including importing of browser cookies on a periodic basis (this may need to be repeated when imported cookies expire.)


The application should also support the following platforms
 * Windows (x86, x64, ARM) [MANDATORY]
 * MacOS (Apple Silicon/ARM) [MANDATORY]
 * Linux (including Ubuntu and other variants) [Good to have]
 * Raspberry Pi [Good to have]


The application should also, wherever possible support the native platform/OS look and design language (such as Liquid Glass on MacOS)

The applications should either embed all required components (including Python, amdecrypt and GAMDL itself) or shouold offer to download these. Ccomponents/libraries should be self-contained to the application, so as to not cause conflict with any other applications on the same device that might use these applications/libraries (versison differencs for example). Having said that, if the appliction itself was "portable" (single file/executable that would be a "nice to have")
For libraries (including GAMDL, Python and any others), the applicataion should regularly check for updates to these libraries, and offer to download an update - however there should be ability to only do so if the newer GAMDL is supported by the UI.

When building the app/project, consider the possibility of adding additional features later, or having a mechanism for 'linking in'/integrating with another app at a later date (we have one in the pipeline/in mind). Also consider having support for downloading music from other platforms (such as YouTube Music - using gytmdl; Spotify - perhaps using votify). These features may not available now, but when working on structure consider this possible expansion, for future releases. Any music services that require special API Keys/cryptographic keys or cookies to work must be stored on the application (whether in settings or within the app) in a secure way, to avoid them being intercepted and used by others, once added/imported using the application settings/preferences UI screens.


The project will be hosted in a Git repot, via GitHub. Please also setup some of the following automation:

* Release/version numbering management
* Release build creation to automate (where possible) creation of downloadable release builds by users
* Setup for managing project milestones, wiki/support articles and reporting/managing issues within GitHub

Where possible, i would prefer a single codebase for all platforms, but if this compromises any of the above requirements, please use different codesbases if necessary!

Where graphics are used in the application, please use vector graphics over others to keep graphic file sizes smaller, but also allow high quality scaling. This also goes for graphics that need to be created.
Any UI elements should also contain support for dark/light mode with automatic switching based on OS settings. Additional theme support can also be added (maybe later?)

There may be other files already in the project folder from previous iterations. I'm happy for these to be overwritten/updated. Any existing files/folders that are no longer required can (and should) be deleted if they do not fit in the new current project (structure).

Application coding should be modular, to help make maintenance simpler, and more maintainable.

PLease produce ALL code with user-readable code formatting (linebrakes, indentation etc), and include DETAILED comments/annotations for code (if possible every line of code) to assist with learning, future code maintenance and code understanding.

Please also include comments in the code including license/copyright and author info.
License should be in line with GAMDL source project and the other components/libraries used. We do not want this project to conflict with any of those.
Copyright year in source code (comments) should be auatomated, and start in 2026, endind in the <current year> with the current year being added/automated in all code without manual intervention.
Copyright should be held with MeedyaDL.


Please device a project plan, and detail this in the Project_Plan.md, and include this plan/overview in README.md
Also create a PROJECT_STATUS.md which will be the go-to place to get the current status, including what's complete, what's still to be worked on etc
Also create a CHANGELOG.md which lists, in detail, every change made, including the date of the change being made and the release date/version number. THis should also be automated, and not need manual intervention.

Markdown (.md) files should be layed-out well and easy to read. Please use text formatting, and even graphics via emojis to help with user-friendly readability.

We also want to create detailed usage documentation, including troubleshooting steps and FAQs. The documentation should be in help/ in Markdown (.md) formaat, but should also have embedded help in the application on each OS platform, in a native way.

Please also keep a record of all CLAUDE prompts, context etc as well as this project brief in .claude so that this is retaained and can allow us to pick up where we left off, keeping project requirements and plan.

Claude files, as well as any .md markdown files (including, but not limited to Project_Plan.md, README.md & CHANGELOG.md) being updated at/after every command/code change automatically to keep project status/to-do lists etc ALWAYS up-to-date

================================================================================
SESSION PROMPTS ARCHIVE
================================================================================
Per the brief above ("keep a record of all CLAUDE prompts, context etc … so that
this is retained and can allow us to pick up where we left off"), each session's
user prompts are appended below in chronological order. The original brief above
this divider is the frozen project genesis and is not edited.

--------------------------------------------------------------------------------
Session: 2026-04-29 — User-reported v0.50.1 fixes (six issues bundled in PR #662)
Branch:  fix/toasts-notifications-fallback-tracebacks-revival
Issues:  #657, #658, #659, #660, #661, #663
PR:      https://github.com/MWBMPartners/MeedyaDL/pull/662
--------------------------------------------------------------------------------

User prompts in this session (paraphrased / verbatim where they materially drove
the work):

1. Initial bug report (three observations from the running v0.50.1 build):
   - "This URL is already in the queue" in-app toast doesn't auto-dismiss; it
     should.
   - MeedyaDL set to "Native + In-app (default)" notification style, but native
     notifications never trigger (at least on macOS).
   - Fallback function works well, but currently you cannot remove some formats
     from the fallback hierarchy. Should be able to remove (or re-add) any
     audio/video format. Some users may not want Binaural for example.
   Asked to: create GitHub Issues for each, fix individually with separate
   commits, and update the issues as each is fixed.

2. Mid-task addendum (two more observations from recent activity logs):
   - Recent downloads showed Python type errors in the Activity Log. Are these
     normal? Errors? Resolvable in MeedyaDL or GAMDL-only?
   - Several instances of timeouts after 10 minutes, with the log marking items
     as failed/complete and then "things to revive again".
   Asked to: create issues + fixes + commits for these too, then open one PR to
   merge to main when all complete.

3. Live evidence escalation:
   - "Another 'Companion download timed out after 10 minutes - marking complete'
     error.... if this happens like before, MeedyaDL will eventually start
     logging errors again." (Predictive — the user knew the symptom would recur
     because no fix had shipped yet.)
   - Follow-up screenshot 11 minutes later: "As predicted, MeedyaDL sprang back
     to life again (at least according to the Activity Log), after 11 minutes or
     so. Check errors on PR 662." (Confirmation that the predicted post-abort
     activity-log chatter happened. Drove the discovery that the lyrics
     conversion is sync code that tokio's JoinHandle::abort cannot preempt — the
     root cause behind issue #663.)
   - Final pattern report: "another timeout (this isn't with the new version
     yet)" with screenshot showing 13-minute timeout. Confirmed the same defect
     recurring; user understood they were on the un-fixed v0.50.1.

4. Housekeeping (this turn):
   - "Update Claude History, Claude Context, Claude Memory and Claude Prompts in
     .claude/. Stage and commit." This appended block is the response.

Key architectural learning preserved in shared memory
(`.claude/memory/project_pr662_user_session_fixes.md`):

- macOS `requestPermission()` only triggers the system prompt the first time
  per bundle ID; if dismissed, every subsequent call resolves silently with
  `'default'` and `sendNotification()` is a no-op. Workaround: run a one-shot
  preflight at app startup and surface the resolved status to the console.
- `tokio::task::JoinHandle::abort()` cannot preempt synchronous code — only
  yields at `.await` points. Any sync function called from inside an async
  tokio task that can outlive the parent must accept and check an
  `Arc<AtomicBool>` cooperative-cancel flag. New `CompanionTaskHandle` wrapper
  bundles the JoinHandle with such a flag for the companion task.
