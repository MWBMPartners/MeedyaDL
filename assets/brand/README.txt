MeedyaDL brand - Graphite "levels" : MAIN KIT (drop-in)
=======================================================
Extract at the REPO ROOT -> files land in public/ and assets/brand/.
(The animated logo rasters ship separately in MeedyaDL-animated-logos.zip
 because of the per-file size limit; extract that at the root too.)

SVG SOURCES (one file each: light + dark + colour-blind; every switch)
  logo.svg  icon.svg (public copy = app-icon.svg)  icon-doc.svg
  logo-liquidglass-light.svg / -dark.svg  (Apple Icon Composer foregrounds)
  Switches: --logo-primary/--logo-secondary/--logo-shadow vars, ?mode=, #hash,
  class="dark"/cb-*, setProperty, + html.theme-dark/theme-light when inlined.
  Self-sufficient: Graphite default, auto OS light/dark.

APP ICONS / FAVICONS (filled tile, per mode)
  icon[-mode].png (1024)  icon[-mode].ico  icon[-mode]-liquidglass.png
  favicon[-mode].ico (16/32/48)  icon-doc.png  icon-doc.ico
  modes: (light) -dark -cb-deutan -cb-deutan-dark -cb-protan -cb-protan-dark
         -cb-tritan -cb-tritan-dark

LAYOUT
  public/        logo.svg, app-icon.svg, favicon.ico
  assets/brand/  SVG sources + icon rasters + README.txt
Unchanged: wordtype.svg, brandkit.html.
