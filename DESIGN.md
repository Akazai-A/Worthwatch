---
name: Worthwatch
description: Your net worth and portfolio, on your own machine.
colors:
  graphite: "#131210"
  graphite-surface: "#1a1917"
  graphite-raised: "#22201c"
  graphite-rule: "#2a2823"
  bone: "#ece7dc"
  bone-muted: "#a39d90"
  bone-faint: "#6a655a"
  brass: "#d4a853"
  brass-wash: "rgba(212, 168, 83, 0.12)"
  clay-loss: "#d98b7a"
  sage-gain: "#9bc49a"
  group-cash: "#3987e5"
  group-investments: "#d95926"
  group-property: "#199e70"
  group-debt: "#9085e9"
  sector-tech: "#5f8fd4"
  sector-financials: "#cc7490"
  sector-consumer: "#3aa68d"
  sector-industrials: "#ad7ec5"
  paper: "#f3efe6"
  paper-surface: "#faf7f0"
  paper-raised: "#ece6d9"
  paper-rule: "#ddd6c7"
  ink: "#1e1c18"
  ink-muted: "#635e52"
  ink-faint: "#9b9484"
  brass-deep: "#9a7420"
  clay-loss-deep: "#b4523f"
  sage-gain-deep: "#4f7f52"
typography:
  display:
    fontFamily: "Newsreader, Georgia, serif"
    fontSize: "4.25rem"
    fontWeight: 500
    lineHeight: 1
    letterSpacing: "-0.01em"
  headline:
    fontFamily: "IBM Plex Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "22px"
    fontWeight: 600
    letterSpacing: "-0.01em"
  title:
    fontFamily: "IBM Plex Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 600
  body:
    fontFamily: "IBM Plex Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.45
  body-small:
    fontFamily: "IBM Plex Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 400
  label:
    fontFamily: "IBM Plex Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 400
    letterSpacing: "0.1em"
  figures:
    fontFamily: "IBM Plex Mono, ui-monospace, SFMono-Regular, Menlo, monospace"
    fontSize: "13px"
    fontWeight: 400
    fontFeature: "tnum"
rounded:
  hair: "2px"
  sm: "4px"
  md: "6px"
  card: "8px"
  dialog: "12px"
  pill: "999px"
spacing:
  xs: "4px"
  sm: "6px"
  md: "10px"
  lg: "12px"
  xl: "16px"
  2xl: "20px"
  page: "28px"
components:
  card:
    backgroundColor: "{colors.graphite-surface}"
    rounded: "{rounded.card}"
    padding: "14px 16px"
  button-primary:
    backgroundColor: "{colors.brass}"
    textColor: "{colors.graphite}"
    rounded: "{rounded.md}"
    padding: "5px 10px"
  button-secondary:
    backgroundColor: "{colors.graphite-surface}"
    textColor: "{colors.bone}"
    rounded: "{rounded.md}"
    padding: "5px 10px"
  segmented-option:
    textColor: "{colors.bone-muted}"
    rounded: "{rounded.md}"
    padding: "5px 11px"
  segmented-option-on:
    backgroundColor: "{colors.brass-wash}"
    textColor: "{colors.bone}"
  input:
    backgroundColor: "{colors.graphite-raised}"
    textColor: "{colors.bone}"
    rounded: "{rounded.md}"
    padding: "5px 8px"
  nav-item:
    textColor: "{colors.bone-muted}"
    rounded: "{rounded.card}"
    padding: "8px 10px"
  nav-item-on:
    backgroundColor: "{colors.brass-wash}"
    textColor: "{colors.bone}"
  pill:
    textColor: "{colors.bone-muted}"
    rounded: "{rounded.pill}"
    padding: "3px 10px"
---

# Design System: Worthwatch

## Overview

**Creative North Star: "Graphite & Brass"**

Worthwatch is a private instrument, not a dashboard. The palette is warm charcoal, with bone-coloured text and a single brass accent. Brass is spent on one thing per screen: the net worth line, the current state, or the one action that matters. Every figure is set in a monospaced face with tabular numerals, so columns of money line up like a ledger. Dark is the home theme. Light is a warm paper version of the same system, not a separate design.

The interface is dense and quiet. Screens are a single centred column, 820px wide (1360px on Changes), made of bordered surfaces on a slightly darker ground. Information is encoded visually wherever it can be: sparklines on every account row, stacked group bars, a waterfall bridge, a treemap, a fan chart, coloured status dots. Words are reserved for labels and the occasional sentence of explanation.

The system is coherent in its colours and figures. It is generic in its scaffolding: the bordered card, the uppercase micro-label and the segmented pill appear on every screen. This document records the system as it ships today, before any redesign.

**Key Characteristics:**
- Warm graphite ground with bone text. Dark-first, with an equal light theme.
- One brass accent, kept rare.
- Fixed group colours: cash blue, investments orange, property green, debt violet. They mean the same thing on every chart.
- Headline figures in the Newsreader serif; every other amount in IBM Plex Mono with tabular numerals. Prose in IBM Plex Sans.
- Flat, bordered surfaces. Depth comes from tone, not shadow.
- Small type: most UI text is 11–13px.

## Colors

A warm, low-chroma neutral system with one metallic accent and four fixed categorical hues.

### Primary
- **Brass** (`brass`, `brass-deep` in light): the net worth line on Home, the active nav item, the selected segment, links, focus rings, and the primary action. **Brass Wash** (`brass-wash`) is its translucent fill for active and selected states.

### Neutral
- **Graphite** (`graphite`): the page ground.
- **Graphite Surface** (`graphite-surface`): cards, panels, tooltips, the palette.
- **Graphite Raised** (`graphite-raised`): inputs, selects, track backgrounds, the active segment in Plan.
- **Graphite Rule** (`graphite-rule`): every 1px border and row divider.
- **Bone** (`bone`): primary text and figures.
- **Bone Muted** (`bone-muted`): secondary text, inactive nav, legends.
- **Bone Faint** (`bone-faint`): micro-labels, ages, axis ticks, hints.
- **Paper, Paper Surface, Paper Raised, Paper Rule, Ink, Ink Muted, Ink Faint**: the same roles in the light theme.

### Semantic
- **Sage Gain** (`sage-gain` / `sage-gain-deep`): positive deltas and fresh-account dots.
- **Clay Loss** (`clay-loss` / `clay-loss-deep`): negative deltas, over-budget meters and errors.
- Warning reuses brass (`--warn` equals `--accent`).

### Categorical
- **Group colours** (`group-cash`, `group-investments`, `group-property`, `group-debt`): the asset-group encoding on Home, Changes, Plan, Account detail and Wall. The light theme uses slightly deeper variants.
- **Sector families** (`sector-tech`, `sector-financials`, `sector-consumer`, `sector-industrials`): used only in the look-through treemap.
- **Fill** and **Fill Soft** (`--fill`, `--fill-soft`): computed mixes of text into surface, used as the one neutral fill for bars and cells that carry no category.

### Named Rules
**The One Brass Rule.** Brass marks the single most important line or state on a screen. Bars and cells without a category use the neutral `--fill`, never brass.

**The Fixed Meaning Rule.** A group colour means its group on every screen. Never reuse cash blue or debt violet for anything else. "What you added" is a bone dashed line, not blue.

**The Hatch Rule for growth.** Growth and paydown (Plan milestones, Account month bars) are brass hatch with a brass edge, so solid brass stays for the net worth line.

## Typography

**Display Font:** Newsreader (500), lining tabular numerals, for headline figures.
**Body Font:** IBM Plex Sans (400, 500, 600), with a system sans fallback.
**Figures Font:** IBM Plex Mono (400, 500), with a system mono fallback.

**Character:** Newsreader gives the headline figure a printed, private-ledger voice. Plex Sans is a quiet grotesque for labels and prose. Plex Mono gives every amount a ledger feel, and tabular numerals keep columns aligned.

### Hierarchy
- **Display** (Newsreader 500, 4.25rem, line-height 1, −0.01em): the headline figure on Home, Portfolio, Changes, Plan, Account and Budget. It drops to 3.25rem below 760px, and rolls like a counter when its value changes. Wall view scales to `clamp(64px, 10vw, 128px)`.
- **Headline** (Plex Sans 600, 22px, −0.01em): screen titles on Check-in, Chat and Account detail.
- **Title** (Plex Sans 600, 20px): empty-state and detail headings.
- **Body** (Plex Sans 400, 14px, line-height 1.45): base text. Explanatory paragraphs are capped near 52ch.
- **Body Small** (12–13px): most row text, controls and legends. 12px is the most common size in the codebase.
- **Label** (Plex Sans 600, 13px, sentence case): section and tile names, set like a ledger heading. Each screen's main label is its h1.
- **Figures** (Plex Mono 400, tabular): every amount, axis tick and percentage, through `.num` and `.money`.

### Named Rules
**The Ledger Rule.** Any number a person might compare is set in the figures face with tabular numerals.

## Layout

- **Shell:** a 180px left rail with a 1px right rule, beside a scrolling main area with 28px padding.
- **Column:** content sits in one centred column, 820px wide (`.col`). Changes widens to 1360px.
- **Rhythm:** sections stack with a 20px gap. Inside cards, gaps are mostly 10–12px. Rows are separated by 1px rules with 7–9px vertical padding.
- **Grids:** Home uses a two-column assets-against-debt bar and a four-up tile row (`repeat(4, 1fr)`). Budget categories use two columns.
- **Responsive:** one breakpoint, at 760px. The rail becomes a horizontally scrolling top bar, main padding drops to 16px, and display figures shrink.
- **Wall view:** a separate full-bleed layout with no rail.

## Elevation & Depth

The system is flat. Depth comes from three tonal steps (ground, then surface, then raised) and a 1px rule around every surface. Shadows appear only on floating layers:
- **Palette dialog** (`box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45)`), over a scrim of `rgba(10, 9, 8, 0.55)` with a 2px backdrop blur.
- **Chart tooltip** (`box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25)`).

### Named Rules
**The Flat Ground Rule.** Resting surfaces never cast shadows. Only layers that float above the page, such as the palette and tooltips, get one.

## Shapes

Corners are softly rounded and small:
- 8px (`--radius`): cards and nav items.
- 6px: buttons, inputs, selects and segmented controls.
- 3–4px: bar tracks and swatches.
- 50%: status dots.
- 999px: pills.
- 12px: the palette dialog.
- Chat bubbles use 12px with one 2px corner.

In practice the codebase uses more than ten distinct radius values. Hatching (a 135° repeating stripe) marks data that is not confirmed yet: Apple Pay taps not on a statement, and undisclosed fund holdings.

## Components

### Navigation (Rail)
A quiet vertical list.
- Each item is a 6px dot plus a label, with 8px by 10px padding and an 8px radius.
- Inactive items are bone-muted. Hover gives a surface background. The active item gets a brass-wash background, bone text and a brass dot.
- Secondary links (Go to… ⌘K, Wall view, Settings, Backups, Blur amounts, Theme) sit at the bottom at 11px faint.
- Focus shows a 2px brass outline with a 2px offset.
- Below 760px the rail becomes a horizontal scroller.

### Cards / Containers
- **Corner Style:** 8px.
- **Background:** graphite surface on the graphite ground.
- **Shadow Strategy:** none (see Elevation & Depth).
- **Border:** 1px graphite rule.
- **Internal Padding:** 14px by 16px.
- **Section labels:** uppercase 11px labels.

### Buttons
Buttons are defined per screen, not shared.
- **Shape:** 6px corners (8px on Home's empty-state call to action).
- **Primary:**
  - Budget, Settings and Home: brass fill with graphite text.
  - Plan and Backups: bone fill with graphite text.
- **Secondary:** a surface or raised fill, a 1px rule border and bone text, with 5px by 10px padding at 12px.
- **Disabled:** 50% opacity.

### Segmented control
Period and mode switches (3M / 1Y / All, Line / By group, Since last / 1M / 3M / YTD / 1Y):
- Options sit in a 1px-ruled container with 6px corners.
- Inactive options are bone-muted. The active option gets brass wash, or a raised fill on Plan.
- The compact `.mini` variant uses figures at 11px.
- Settings uses an 8px-corner variant with dividers between options.

### Inputs / Fields
- **Style:** raised fill, a 1px rule border, 6px corners and 5px by 8px padding at 13px. Selects match.
- **Focus:** the browser default outline, except on elements that set their own brass ring.

### Pills and chips
- **Pills:** 999px corners at 11px. A 1px rule when inactive, brass wash when on.
- **Budget chips:** bordered 8px surfaces that hold a dot, text and an inline control.
- **Delta chips:** a figure plus a muted caption in a baseline-aligned inline flex, shown under the net worth figure.

### Signature components
- **Money:** a component for every amount. It uses the figures face, colours signs as gain or loss, and blurs under ⌘⇧H.
- **Sparkline:** an 80px-wide line on each account row, ending in a brass dot.
- **Net worth chart (uPlot):** a brass line over a faint brass area, with mono axis ticks. The By group view stacks group colours.
- **Assets-against-debt bar:** a split track in group colours with a legend.
- **Bridge:** the Changes waterfall of market moves, contributions, debt paid down and FX.
- **Treemap:** look-through tiles in sector families. Undisclosed holdings are hatched.
- **Fan chart:** the Plan projection band.
- **DeltaBar and SplitStrip:** small inline bars for change and composition.
- **Status dot:** a 7px dot for account freshness (sage fresh, brass stale, faint none) or transaction state (hollow for a tap, filled when matched).
- **Command palette:** a 560px floating dialog at 12vh, opened with ⌘K.

## Motion

Motion explains a change; it never decorates a resting screen. Every curve is `--ease-out`
(`cubic-bezier(0.16, 1, 0.3, 1)`), with no bounce.

- **Headline figures roll** like a counter when their value changes (a check-in, a price, a scrubbed day). 420ms.
- **Bars grow from their baseline** on arrival: share bars from the left, delta bars out of the zero line, a row at a time (30ms apart, capped).
- **The Changes bridge builds** one step at a time, each bar growing from where the last one ended.
- **Charts draw in** from the left when the range or view changes.
- **The rail bookmark slides** to the current screen.
- **The check-in stamp lands** once this month's check-in is in; the Undo bar rises; look-through ribbons draw in from the funds.
- **Reduced motion** keeps every state change and drops the movement: animations jump to their end, and transitions keep only colour, opacity and filter.

### Chart hover

Hover never puts a dot or a floating box on the line it describes. A **caliper** measures the hovered day: a
dashed hairline, a brass bracket from the reference line to net worth, and the gain it spans beside it
(what the market added on Added vs grown, otherwise the gain since the window opened). The date and value are
tagged on the axes, and the full reading goes in the line under the chart. Plan's projection marks the hovered
month with a short brass level mark.

## Do's and Don'ts

### Do:
- **Do** set every amount in `var(--figures)` with tabular numerals.
- **Do** take colours only from `tokens.css`. Components contain no hex literals.
- **Do** keep brass for the one primary line, state or action on a screen. Use `--fill` for neutral bars.
- **Do** use the fixed group colours for cash, investments, property and debt on every chart.
- **Do** mark unconfirmed data with the 135° hatch, not a new colour.
- **Do** design both themes. Every token has a light value.
- **Do** respect `prefers-reduced-motion`.

### Don't:
- **Don't** add shadows to resting cards or rows.
- **Don't** use a sector colour outside the treemap, or a group colour for anything but its group.
- **Don't** introduce a second accent hue.
- **Don't** write "worthwatch" in lowercase in visible UI. The brand is "Worthwatch".
