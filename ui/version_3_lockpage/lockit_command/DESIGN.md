# Design System Specification: High-Precision Brutalism

## 1. Overview & Creative North Star
**Creative North Star: "The Sovereign Console"**

This design system is a rejection of the "soft" web. It moves away from the approachable, rounded, and gradient-heavy aesthetics of consumer SaaS to embrace a high-precision, military-grade interface. It is designed for the operator—the developer or security professional who demands surgical clarity and zero latency in their visual environment.

The system utilizes **Architectural Brutalism**: a philosophy where the structure is the decoration. We achieve a premium feel not through shadows or embellishments, but through perfect alignment, intentional "ink-heavy" typography, and a rigid adherence to a 1px world. This is a "hard-surface" UI that feels like it was milled from a single block of steel.

---

## 2. Colors
The palette is intentionally stark, leveraging high-contrast ratios to signify authority and urgency.

### Core Palette
- **Primary (`#000000`)**: Used for all structural elements, primary actions, and "Ink" (text). 
- **Surface (`#FFFFFF`)**: The "Canvas." A pure, clinical white that provides the highest possible contrast.
- **Secondary/Accents**:
    - **Dark Crimson (`#8B0000`)**: Reserved for destructive actions, critical security breaches, or "Armed" states.
    - **Burnt Orange (`#CC5500`)**: Used for warnings, active process indicators, and high-priority data points.

### The "Surgical Border" Rule
Unlike traditional editorial systems that use background shifts, this system defines boundaries through **explicit containment**. 
- **Rule:** Every distinct functional area must be bound by a `1px` solid border (`#000000` or `#C6C6C6`). 
- **Nesting:** To create depth, we do not use shadows. We use "Inset Containers." A `surface_container` (`#EEEEEE`) nested within a `surface` (`#FFFFFF`) must be separated by a sharp `1px` black border. This mimics the look of a technical schematic.

### Glass & Texture
- **No Gradients:** Gradients are strictly prohibited. Movement is conveyed through solid color state changes.
- **Micro-Textures:** For large empty states, a 1px dot grid pattern (Primary at 5% opacity) may be used to reinforce the "blueprint" aesthetic.

---

## 3. Typography
The typographic system is a duality between human-readable instruction and machine-executable data.

| Level | Font Family | Weight | Size | Tracking | Case |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Display** | Inter | 800 (Extra Bold) | 3.5rem | -0.04em | Sentence |
| **Headline** | Inter | 700 (Bold) | 2.0rem | -0.02em | Sentence |
| **Title** | Inter | 600 (Semi-Bold) | 1.125rem | 0 | Sentence |
| **Body (UI)** | Inter | 400 (Regular) | 0.875rem | 0 | Sentence |
| **Data/Code** | JetBrains Mono | 500 (Medium) | 0.875rem | -0.01em | None |
| **Label/Meta** | JetBrains Mono | 700 (Bold) | 0.6875rem | +0.05em | ALL CAPS |

**Identity Logic:** Inter provides the "Professional" veneer, while JetBrains Mono handles the "Surgical Precision." All interactive data fields, timestamps, and hash strings MUST be rendered in JetBrains Mono to signify they are machine-generated.

---

## 4. Elevation & Depth: The "Flat-Stack" Principle
We reject the concept of the Z-axis (height). Instead, we use **Tonal Layering and Insets**.

- **The Layering Principle:** Depth is achieved by stacking 1px boxes. To elevate an element (like a modal), we do not use a shadow; we use a "Double Border" or an offset black background block (1px-2px offset) to create a "shadow-mimic" using a solid shape.
- **Ambient Shadows:** Prohibited. 
- **Ghost Borders:** Used for disabled states or secondary groupings. Use `outline_variant` (`#C6C6C6`) at 100% opacity. 
- **Rounding:** The `DEFAULT` is `0px`. A maximum of `2px` is permitted only for micro-components like checkboxes or status pips to prevent "visual vibration" on high-DPI screens.

---

## 5. Components

### Buttons
- **Primary:** Black background, White JetBrains Mono text. `0px` radius. `1px` black border.
- **Secondary:** White background, Black text. `1px` black border.
- **Critical:** Dark Crimson (`#8B0000`) background, White text.
- **Hover State:** Immediate color inversion (e.g., Primary becomes White bg/Black text) with no transition timing. It should feel instantaneous.

### Input Fields
- **Default:** White background, `1px` border (`#777777`). 
- **Focus:** `1px` black border with a `2px` internal "Focus Ring" (a solid orange or crimson line inside the border).
- **Typography:** All user input uses JetBrains Mono.

### Cards & Layouts
- **The "Technical Sheet":** No dividers. Use a `1px` border for the entire card. If sections are needed within the card, use a header bar with a black background and white title text to "cap" the section.
- **Spacing:** Use a strict 4px/8px grid. Alignment must be pixel-perfect; any misalignment breaks the "military-grade" promise.

### New Component: The "Status Bar"
A persistent 24px tall bar at the bottom of containers or the screen. 
- **Style:** Black background, Orange or Crimson text in JetBrains Mono (Label-sm). 
- **Purpose:** Displays system health, "Live" connection status, or encryption keys.

---

## 6. Do's and Don'ts

### Do
- **Do** align all text to the top-left of its container to maintain a "terminal" feel.
- **Do** use "All Caps" for labels and metadata to imply a sense of urgency.
- **Do** use 1px borders to separate navigation from content—never use shadows.
- **Do** treat whitespace as a functional separator. If two elements are related, they share a border; if they are not, they are separated by exactly 32px of pure white space.

### Don't
- **Don't** use animations like "Ease-in-out." If an element must move, use "Linear" or no transition at all.
- **Don't** use icons with rounded terminals. Use sharp, geometric iconography (1px stroke width).
- **Don't** use gray text for body copy. Use `#000000` for maximum legibility. Only use Grays for non-functional decorative lines or disabled states.
- **Don't** add "Padding-top" to display headers—let them sit tight against the top border of a container to emphasize the brutalist structure.