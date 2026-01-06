---
name: brand-guidelines
description: Applies BlackBear TechHive's official brand colors, typography, and design principles to any artifact. Use when brand colors, style guidelines, visual formatting, or company design standards apply.
license: Complete terms in LICENSE.txt
---

# BlackBear TechHive Brand Styling

## Overview

To access BlackBear TechHive's official brand identity and style resources, use this skill.

**Keywords**: branding, corporate identity, visual identity, post-processing, styling, brand colors, typography, BlackBear brand, visual formatting, visual design, logo guidelines

**Reference**: `doc/refering/blackbear_brand_guideline.txt` (Brand Guidelines October 2022)

## Brand Strategy

**Guiding Your Path To Digitalization**

We offer value-added specific or integrated solutions targeting utilities, automation and digitalization for:
- Logistics
- Networking
- Manufacturing
- Cybersecurity

With our own engineering and manufacturing teams, we:
- Consult
- Design
- Manufacture
- Support

## Design Principles

The BlackBear TechHive brand is visually represented by two core brand elements:
- **The bear icon**: Formosan Black Bear (God of Mountains for Taiwanese indigenous peoples)
- **Logotype**: Bold, simple, and direct

**Three Core Design Principles:**
1. **Flexible**: Adaptable to various formats and contexts
2. **Growth**: Scalable and expandable design system
3. **Contrast**: Strong visual distinction and clarity

## Logo Guidelines

### Logo Versions

**Primary Logo - Horizontal**
- Use to build recognition into the BlackBear TechHive brand
- Best for standard applications

**Secondary Logo - Vertical**
- Works best in extreme formats
- Use when horizontal space is limited

**Third Logo - Icon (Bear only)**
- Bold and immediately recognizable signifier
- **IMPORTANT**: When using only the bear icon, the website `blackbeartechhive.com` must be shown

### Logo Colors

**Two-Color Logo (Primary):**
- **Black & Yellow ONLY**
- Do NOT use other colors, gradients, gray, or patterns within the logo
- Can only be shown on white or light gray background

**Black Color Logo:**
- Used ONLY for partnering
- Can only be shown on yellow or white background

### Clear Space

- Minimum clear space: ½ x (where x = height of the icon)
- Give the logo adequate breathing room

## Color System

The BlackBear color system portrays confidence, innovation, and professionalism.

### Primary Colors

**Black**
- RGB: `0, 0, 0`
- Hex: `#000000`
- Usage: Primary text, strong contrasts, logo

**Yellow**
- RGB: `250, 199, 0`
- Hex: `#E8C236`
- Usage: Primary accent, brand highlight, logo

### Neutral Colors

**Dark Gray**
- RGB: `167, 168, 169`
- Hex: `#A7A8A9`
- Usage: Secondary text, subtle elements

**Warm Gray**
- RGB: `215, 210, 203`
- Hex: `#D7D2CB`
- Usage: Backgrounds, neutral tones

**Blue Gray**
- RGB: `221, 229, 237`
- Hex: `#DDE5ED`
- Usage: Light backgrounds, subtle highlights

**Cool Gray**
- RGB: `204, 204, 204`
- Hex: `#CCCCCC`
- Usage: Borders, dividers

**White**
- RGB: `255, 255, 255`
- Hex: `#FFFFFF`
- Usage: Clean backgrounds, light text on dark

### Subsidiary Colors

**Yellow Light**
- RGB: `238, 192, 73`
- Hex: `#EEC049`

**Green**
- RGB: `102, 204, 51`
- Hex: `#66CC33`

**Teal**
- RGB: `51, 153, 153`
- Hex: `#339999`

**Magenta**
- RGB: `235, 15, 255`
- Hex: `#EB0FFF`

**Brown**
- RGB: `70, 41, 17`
- Hex: `#462911`

**Cyan**
- RGB: `51, 204, 255`
- Hex: `#33CCFF`

**Dark Green**
- RGB: `51, 153, 51`
- Hex: `#339933`

## Typography

The BlackBear typeface represents our personality: **bold, simple, and direct**.

### Brand Typeface

**Krona One**
- Usage: Brand-led titles, must-highlight messaging
- Weight: Regular
- Characteristics: Bold, distinctive, memorable

### Fallback Font

**Arial**
- Usage: When Krona One is unavailable
- Weights: Regular, Italic, Bold
- Rationale: Available universally, similar bold characteristics

### Additional Typeface (Multi-language)

**Noto Sans**
- Usage: Chinese Traditional, Chinese Simplified, Japanese, and other languages
- Weights: Regular, Medium, Bold only
- Example: 台灣黑熊網路安全

### Typographic Hierarchy

**Title One:**
- Font: Krona One
- Size: 50pt
- Line Height: 55pt
- Tracking: -25

**Title Two:**
- Font: Arial Bold
- Size: 68pt
- Line Height: 75pt
- Tracking: 0

**Subtitle One:**
- Font: Arial Bold
- Size: 32pt
- Line Height: 35pt
- Tracking: 0

**Subtitle Two:**
- Font: Arial Regular
- Size: 38pt
- Line Height: 42pt
- Tracking: 0

## Features

### Smart Font Application

- Applies Krona One font to brand titles and key messaging
- Applies Arial for body text and hierarchy
- Automatically falls back to Arial if Krona One unavailable
- Supports multi-language with Noto Sans
- Preserves readability across all systems

### Color Application Strategy

**Primary Use:**
- Black (`#000000`) for text, strong elements
- Yellow (`#E8C236`) for highlights, accents, CTAs

**Secondary Use:**
- Neutral grays for backgrounds and subtle elements
- Subsidiary colors for specific contexts (icons, graphics, data visualization)

### Logo Usage Rules

1. Always use official logo versions (horizontal, vertical, or icon)
2. Maintain two-color requirement (Black & Yellow)
3. Never apply gradients, patterns, or unauthorized colors
4. Respect clear space requirements (½ x minimum)
5. When using icon alone, include website URL

## Technical Details

### Font Management

- Primary: Krona One (for titles)
- Fallback: Arial (universal availability)
- Multi-language: Noto Sans (CJK support)
- No font installation required for fallback
- For best results, pre-install Krona One and Noto Sans

### Color Application

- Uses RGB color values for precise brand matching
- Applied via python-pptx's RGBColor class or CSS hex codes
- Maintains color fidelity across different systems
- Primary palette (Black + Yellow) for core brand elements
- Extended palette (neutrals + subsidiary) for flexible applications

## Brand Spirit

The **Formosan Black Bear** symbolizes:
- **Professional**: Expert in our domains
- **Experienced**: Proven track record
- **Flexible**: Adaptable to client needs
- **Innovative**: Value innovation above all
- **Ethical**: Strong ethical foundation

"Without bears, the forest is an empty dwelling." - Bunun hunters

We source and engineer efficient solutions tailored for each customer.
