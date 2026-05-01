# Design System Specification: Technical Brutalism

## 1. Overview & Creative North Star: "The Analog Terminal"
The Creative North Star for this design system is **The Analog Terminal**. It moves away from the "softness" of modern SaaS and leans into the uncompromising precision of high-end industrial hardware and terminal interfaces. 

This is not a "friendly" system; it is an **authoritative** one. By utilizing a zero-radius (0px) architecture, high-density layouts, and a stark monochromatic base, we create an environment of extreme clarity. We break the "template" look through intentional asymmetry—utilizing heavy 1px black borders as structural anchors and "JetBrains Mono" to elevate data from mere information to a primary aesthetic feature. It feels like a high-end physical console translated into a digital light-mode experience.

---

## 2. Colors
The palette is rooted in absolute contrast. While the background is a surgical `#FFFFFF`, the system utilizes a "Surface Tier" to define hierarchy without losing its high-density utilitarian edge.

### Core Palette
*   **Primary (`#000000`)**: Used for all structural borders, primary headings, and active states. It is the "ink" on the page.
*   **Secondary (`#B34700` / Industrial Orange)**: Reserved for technical highlights, active CLI prompts, and warnings. It signifies "System Logic."
*   **Tertiary (`#A30000` / Tactical Red)**: Reserved for destructive actions and critical system failures. It signifies "Physical Risk."

### Surface & Tonal Layering
*   **Surface (`#FFFFFF`)**: The base canvas.
*   **Surface-Container-Low (`#F3F3F3`)**: Used for background groupings and sidebars.
*   **Surface-Container-Highest (`#E2E2E2`)**: Used for header strips or inactive tab backgrounds.

**The "High-Contrast Border" Rule:**
Unlike traditional soft-UI systems, this system **mandates** 1px solid `#000000` borders for primary sectioning. Do not use background shifts alone to define major regions; use a hard black line to "contain" the logic. 

**The "Glass & Texture" Exception:**
While the system is utilitarian, "Glassmorphism" is used specifically for floating command palettes or "hovering" terminal overlays. Use `surface_container` at 80% opacity with a `backdrop-blur` of 12px to maintain legibility while suggesting a layered software stack.

---

## 3. Typography
The system uses a dual-typeface approach to distinguish between "User Interface" and "System Data."

*   **UI Labels & Navigation (Inter)**: Clean, sans-serif, and neutral. Used for menus, buttons, and settings labels.
*   **Data & Terminal (JetBrains Mono)**: The "Soul" of the system. Used for all input fields, log outputs, metrics, and technical values.

### Typography Scale
| Level | Font | Size | Weight | Case |
| :--- | :--- | :--- | :--- | :--- |
| **Display-LG** | Inter | 3.5rem | 800 | Sentence |
| **Headline-SM** | Inter | 1.5rem | 700 | Sentence |
| **Title-SM** | Inter | 1.0rem | 600 | ALL CAPS |
| **Body-MD** | Inter | 0.875rem | 400 | Sentence |
| **Label-MD** | JetBrains Mono | 0.75rem | 500 | ALL CAPS |
| **Code-Data** | JetBrains Mono | 0.875rem | 400 | N/A |

---

## 4. Elevation & Depth: The Stacking Principle
In this system, depth is not achieved through light and shadow, but through **Industrial Stacking**.

*   **The Layering Principle**: Treat the UI like stacked sheets of technical drawings. A card doesn't "float" with a shadow; it sits on top of a section, defined by a 1px `#000000` border and perhaps a 2px offset "Black-Block Shadow" (a solid black rectangle offset by 2px/2px) for high-priority items.
*   **Shadows**: Strictly prohibited for standard elements. If a floating element (like a Tooltip) requires separation, use a 1px `#000000` border with a subtle `surface_container_low` background.
*   **Zero-Rounding**: Every element—buttons, cards, inputs, and windows—must have a **0px border-radius**. Sharp edges are non-negotiable.

---

## 5. Components

### Buttons
*   **Primary**: Solid `#000000` fill with `#FFFFFF` Inter Bold text. 0px radius.
*   **Secondary**: `#FFFFFF` fill, 1px `#000000` border, `#000000` Inter Bold text.
*   **Technical (Warning)**: Solid `#B34700` fill, `#FFFFFF` JetBrains Mono text.
*   **Interaction**: On hover, Primary buttons invert (White fill, Black border/text).

### Input Fields
*   **Styling**: 1px `#000000` border, `#FFFFFF` background.
*   **Font**: Always JetBrains Mono.
*   **States**: Focus state uses a 2px Industrial Orange (`#B34700`) bottom-border "accent line" to indicate the active cursor.

### Cards & Data Tables
*   **Structure**: No dividers within tables. Use vertical white space and JetBrains Mono for all cell data.
*   **Headers**: Table headers should have a `#000000` background with `#FFFFFF` JetBrains Mono text (Label-SM).

### The "CLI Command" Component
A specialized component for this system: A horizontal strip using `surface_container_highest` (`#E2E2E2`) with a prompt symbol `>` in Industrial Orange. This is the primary way users interact with system-level commands.

---

## 6. Do's and Don'ts

### Do
*   **DO** use JetBrains Mono for any value that can be toggled, edited, or monitored.
*   **DO** utilize "All Caps" for Title-SM and Label-MD to lean into the technical blueprint aesthetic.
*   **DO** use asymmetric layouts (e.g., a left-heavy sidebar with a high-density right-side data grid).

### Don't
*   **DON'T** use border-radius. Even a 1px radius violates the system's integrity.
*   **DON'T** use soft grey borders. If a border exists, it is `#000000` or it is nothing.
*   **DON'T** use standard "Success Green." Use the Industrial Orange for all non-critical active system states.
*   **DON'T** use gradients. Depth is created via solid color blocks and 1px lines only.