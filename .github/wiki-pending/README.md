# Wiki changes waiting to be applied

The cloud sessions can read the GitHub wiki but are not allowed to push to it. Wiki
changes they make are saved here as patches, to be applied from a workstation:

```bash
git clone https://github.com/MWBMPartners/MeedyaDL.wiki.git
cd MeedyaDL.wiki
git am /path/to/MeedyaDL/.github/wiki-pending/2026-11-17-wiki-refresh.patch
git push
```

Once a patch has been pushed to the wiki, delete it from this folder.

- `2026-11-17-wiki-refresh.patch` — brings the wiki up to date: four release channels
  (Nightly, Weekly and Monthly are gone), current features, troubleshooting and wrapper
  settings in plain English, and removes the developer-access key sequence the
  pre-release page used to publish.
