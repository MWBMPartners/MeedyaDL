<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Audio Codecs

Understanding the differences between audio codecs helps you choose the right balance between quality, file size, and device compatibility. This guide explains each option in plain language.

---

## Reliability Notice

Most audio codecs are marked **(Experimental)** in the codec selector. This means they may fail intermittently when using cookie-based authentication. **On GAMDL versions before 3.8**, only two codecs are reliably downloadable without the Wrapper service:

- **AAC Legacy** (256kbps at 44.1kHz) — reliable with cookies
- **AAC-HE Legacy** (64kbps) — reliable with cookies

### On GAMDL 3.8 and newer, most of that changes

GAMDL 3.8 added a new HLS asset endpoint that lets every codec except **ALAC (Lossless)** download with cookie-based authentication alone — including **Dolby Atmos** and **AC3**, which previously needed the Wrapper on every earlier release. Codecs are still labelled **(Experimental)** regardless of GAMDL version — they can still fail intermittently — but on GAMDL 3.8+ only ALAC actually depends on the Wrapper for reliable downloads. See [Wrapper Authentication](wrapper.md) for the full version-by-version breakdown.

All other codecs — including ALAC (Lossless), Dolby Atmos, AC3, AAC, and AAC Binaural — depend on DRM key exchange that cookies don't always handle correctly. If you experience download failures with experimental codecs, consider:

1. **Retrying** — failures are intermittent, a retry may succeed
2. **Enabling the fallback chain** — Settings > Fallback lets MeedyaDL automatically try the next codec
3. **Using the Wrapper service** (needed for ALAC on GAMDL 3.8+; needed for the full non-web codec set on GAMDL 3.0–3.7.x) — provides more reliable access (Linux x86_64 only, see Help > Wrapper)

---

## The Main Codecs Explained

### ALAC — Lossless (Apple Lossless Audio Codec)

ALAC is the highest-quality audio option. It compresses audio without losing any data — the decoded audio is identical to the original studio master. Think of it like a ZIP file for music: smaller than the raw source, but nothing is thrown away.

- **Quality:** Bit-for-bit identical to the source. Available in CD quality (16-bit/44.1kHz), studio quality (24-bit/48kHz), Hi-Res (24-bit/96kHz), and maximum resolution (24-bit/192kHz)
- **File size:** ~5 MB/min (CD quality) to ~15 MB/min (24-bit/192kHz) — roughly 2.5–7× larger than AAC
- **Compatibility:** All Apple devices, iTunes, and many third-party players. Some non-Apple devices may need conversion to FLAC
- **Best for:** Audiophile listening, high-quality speakers/headphones, archival. If you want the absolute best quality and have the storage space, this is the one to choose

### Dolby Atmos — Spatial Audio

Dolby Atmos is an immersive audio format that places sounds in 3D space around you. Instead of traditional stereo (left/right), Atmos positions individual instruments and sounds as "objects" that your playback system renders all around and above you. The result is a more enveloping, cinematic listening experience.

- **Quality:** Depends on the spatial mix — can be stunning on compatible hardware. Encoded as Enhanced AC-3 (EC-3)
- **File size:** Varies by complexity of the spatial mix
- **Compatibility:** Requires Atmos-compatible hardware for the full experience — AirPods Pro, AirPods Max, AirPods 3rd gen+, Dolby Atmos soundbars, AV receivers, and supported speakers. On unsupported devices, it plays as a standard stereo or surround downmix
- **Best for:** Listening through AirPods Pro/Max or a Dolby Atmos home theatre. If you have compatible headphones, Atmos tracks can sound dramatically more spacious and immersive than stereo

### AC3 — Dolby Digital (Surround Sound)

AC3 (also called Dolby Digital) is the classic surround-sound format used in DVDs and home theatres since the 1990s. It delivers up to 5.1 channels: front left, centre, front right, surround left, surround right, plus a subwoofer channel.

- **Quality:** Lossy compression, but designed for surround sound with up to 5.1 channels
- **File size:** Moderate — roughly comparable to AAC
- **Compatibility:** Universally supported by AV receivers, soundbars, and home theatre systems. Less common on phones and portable devices
- **Best for:** Playing through a traditional surround-sound speaker setup (5.1 or 7.1). If you have an AV receiver or soundbar, AC3 will give you multichannel audio without needing Atmos hardware

### AAC — Standard (256 kbps)

AAC (Advanced Audio Coding) at 256 kbps is Apple Music's standard lossy format. It discards audio data that is theoretically inaudible to achieve much smaller file sizes. At 256 kbps, most listeners cannot distinguish it from lossless in a blind test.

- **Quality:** Very good. Transparent to most listeners in everyday environments
- **File size:** ~2 MB/min — the smallest files of the main codecs
- **Compatibility:** Universal. Plays on every device, operating system, browser, and media player
- **Best for:** Everyday listening, phones, portable devices, limited storage. This is the sensible default if you don't have strong feelings about audio quality

### AAC Binaural

AAC Binaural takes a Dolby Atmos or spatial audio mix and renders it as a two-channel stereo signal specifically processed for headphone listening. It simulates the 3D positioning of Atmos using psychoacoustic techniques (head-related transfer functions), so you hear spatial depth and width through ordinary stereo headphones.

- **Quality:** 256 kbps lossy, but with spatial processing applied. Not the same as standard stereo — it's designed to trick your ears into perceiving surround sound
- **File size:** Similar to standard AAC (~2 MB/min)
- **Compatibility:** Plays on any device as a standard stereo .m4a file
- **Best for:** Experiencing spatial audio through regular wired or wireless headphones that don't support Atmos natively. If you want the "immersive" feel but your headphones aren't AirPods Pro/Max, this is the next best thing

### AAC Legacy (256 kbps, 44.1 kHz)

An older AAC encoding profile capped at 44.1 kHz sample rate. Functionally identical to standard AAC for most content, but uses a legacy encoding path designed for maximum compatibility with vintage hardware.

- **Best for:** Older iPods, early-generation media players, or any device that struggles with standard AAC. Only use this if you have playback issues on older equipment

### Experimental Codecs

All codecs except **AAC Legacy** and **AAC-HE Legacy** are marked as **(Experimental)**. This includes the main codecs above (ALAC, Dolby Atmos, AC3, AAC, AAC Binaural) as well as the following niche variants:

- **AAC-HE** — High Efficiency AAC at ~48–96 kbps. Much smaller files but audibly lower quality
- **AAC Downmix** — Surround-to-stereo downmix without binaural processing (a "flat" stereo fold-down)
- **AAC-HE Binaural** — AAC-HE combined with binaural rendering
- **AAC-HE Downmix** — AAC-HE combined with stereo downmix

The "Experimental" label indicates that these codecs may fail intermittently when using cookie-based authentication. The Wrapper service provides more reliable access to all codec types — see Help > Wrapper for details.

---

## Pros & Cons Comparison

| Codec | Pros | Cons |
| ----- | ---- | ---- |
| **ALAC (Lossless)** | Perfect quality, no data lost; supports Hi-Res up to 24-bit/192kHz; great for archival | Large files (2.5–7× bigger than AAC); requires more storage; overkill for casual listening |
| **Dolby Atmos** | Immersive 3D spatial audio; stunning on compatible hardware; reveals details stereo cannot | Requires Atmos-compatible headphones/speakers for full effect; falls back to flat stereo on unsupported devices; not all tracks have Atmos mixes |
| **AC3 (Dolby Digital)** | True multichannel surround (5.1); universally supported by home theatre gear | Lossy compression; limited to 5.1 channels; not useful on phones/headphones; fewer tracks available in AC3 than AAC |
| **AAC (256 kbps)** | Universal compatibility; tiny files (~2 MB/min); indistinguishable from lossless for most listeners | Lossy — discards some audio data permanently; not ideal for archival or high-end listening |
| **AAC Binaural** | Simulated spatial audio through any stereo headphones; same small file size as AAC | Lossy; spatial simulation is approximate — not as good as native Atmos on compatible hardware; only useful with headphones, not speakers |

---

## Which Should I Choose?

There are two main goals when choosing a codec, and each has its own recommended setup:

### Recommendation 1: Best Raw Audio Quality → ALAC (Lossless)

If your priority is **pure audio fidelity** — the highest quality, bit-for-bit identical reproduction of the original studio master — choose **ALAC** as your default codec and enable the **fallback chain** in Settings > Quality.

ALAC preserves every detail of the original recording with no data lost. It supports Hi-Res up to 24-bit/192kHz, making it ideal for audiophile listening, high-quality speakers and headphones, and archival. The fallback chain ensures that when lossless isn't available for a particular track, MeedyaDL automatically tries the next codec in your chain (e.g., AAC 256 kbps) so you always get a download.

**Choose ALAC if:** you listen on quality speakers or headphones, you want the best your equipment can reproduce, or you want to build a future-proof archive. ALAC files are larger (2.5–7× bigger than AAC), so make sure you have the storage space.

### Recommendation 2: Immersive Spatial/Multichannel Audio → Dolby Atmos

If your priority is **immersive, three-dimensional audio** — hearing instruments and sounds placed all around you in 3D space — choose **Dolby Atmos** as your default codec and enable the **fallback chain**.

Dolby Atmos uses object-based mixing to position sounds in 3D space rather than just left/right stereo. On compatible hardware (AirPods Pro, AirPods Max, Atmos soundbars, compatible AV receivers), the result is a dramatically more spacious and enveloping listening experience. The fallback chain is especially important here because not every track has an Atmos mix — when Atmos isn't available, MeedyaDL will automatically fall back through AC3 (5.1 surround), AAC Binaural (simulated spatial for regular headphones), and then standard AAC.

**Choose Atmos if:** you have AirPods Pro/Max, a Dolby Atmos soundbar, or a compatible home theatre system, and you want the most immersive listening experience available. Consider enabling a **Companion Download** of ALAC (in Settings > Quality) so you also get a lossless copy of every track alongside the Atmos version.

### Which One Is Right for Me?

| Priority | Recommended Codec | Why |
| -------- | ----------------- | --- |
| Raw quality, perfect reproduction | **ALAC** | Bit-for-bit identical to the studio master. No data lost. Best for quality speakers, headphones, and archival |
| Immersive spatial/3D audio | **Dolby Atmos** | 3D object-based positioning. Instruments surround you. Best for AirPods Pro/Max and Atmos systems |

If you care about **both**, set Dolby Atmos as your default with a Companion Download of ALAC. You'll get the spatial experience on compatible tracks and a lossless backup for everything.

### Other Scenarios

- **"I have a surround-sound system but not Atmos"** → Choose **AC3** (Dolby Digital) for 5.1 multichannel content.
- **"I just want it to work everywhere"** → Choose **AAC** (256 kbps). Smallest files, plays on everything, sounds great.
- **"I want spatial audio but my headphones aren't AirPods"** → Choose **AAC Binaural**. You'll get simulated 3D audio through any standard headphones.

For more details on fallback behaviour, see the Fallback Quality section.
