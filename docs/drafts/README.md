<!-- Copyright (c) 2024-2026 MeedyaSuite. Licensed under the MIT License. -->

# Working drafts

Documents that are still being written and are not part of the app.

They live here, not in `help/`, because everything in `help/` is copied
into every installer (`tauri.conf.json` bundles `../help/*` as
resources). Until October 2026 the three Word drafts of the Docker
wrapper guide sat in `help/`, so every MeedyaDL installer carried them.

The polish check (`tools/audit-checks/check_polish.py`) refuses Word
files, drafts and backups in `help/` and `public/`, so they cannot drift
back.

| File | What it is |
|---|---|
| `docker-wrapper-gamdl-guide.docx` | First draft of a guide to running the wrapper in Docker on another machine |
| `docker-wrapper-gamdl-guide-v2.docx` | Second draft of the same guide |
| `docker-wrapper-gamdl-guide-v3.docx` | Third draft of the same guide |
