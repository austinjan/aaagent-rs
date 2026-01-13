# Frontend Development Guide

This guide documents the frontend architecture, design principles, and best practices for the aaagent-rs web interface.



## Architecture Overview

**Tech Stack:**
- **Framework:** React 18 + TypeScript
- **Build Tool:** Vite
- **Styling:** Tailwind CSS v4 (CSS-first)
- **UI Components:** shadcn/ui
- **Icons:** lucide-react
- **Routing:** react-router-dom

**Deployment:** Embedded in Rust binary via `rust-embed`

## Project Structure

```
web/
├── src/
│   ├── components/
│   │   ├── chat/              # Chat-related components
│   │   │   ├── MessageCard.tsx
│   │   │   ├── ThinkingBlock.tsx
│   │   │   ├── ToolCallCard.tsx
│   │   │   ├── ChatContainer.tsx
│   │   │   └── ChatInput.tsx
│   │   ├── config/            # Configuration UI
│   │   │   └── ConfigPanel.tsx
│   │   └── ui/                # shadcn/ui base components
│   │       ├── button.tsx
│   │       ├── button-variants.ts
│   │       ├── select.tsx
│   │       └── slider.tsx
│   ├── pages/                 # Page components
│   │   ├── Home.tsx
│   │   ├── Chat.tsx
│   │   ├── Testing.tsx
│   │   └── MessageCardDemo.tsx
│   ├── lib/
│   │   └── utils.ts           # Utility functions (cn, etc.)
│   ├── App.tsx                # Root component with routing
│   ├── index.css              # Global styles and Tailwind config
│   └── main.tsx               # Entry point
├── dist/                      # Production build (embedded in binary)
├── tailwind.config.js         # Tailwind configuration
├── vite.config.ts             # Vite configuration
└── package.json
```

## Design System

### Color Palette

All colors are defined as CSS variables in HSL format in `src/index.css`:

```css
:root {
  /* Base colors */
  --background: 0 0% 100%;
  --foreground: 0 0% 3.9%;
  --card: 0 0% 100%;
  --card-foreground: 0 0% 3.9%;
  --popover: 0 0% 100%;
  --popover-foreground: 0 0% 3.9%;
  --primary: 0 0% 9%;
  --primary-foreground: 0 0% 98%;
  --secondary: 0 0% 96.1%;
  --secondary-foreground: 0 0% 9%;
  --muted: 0 0% 96.1%;
  --muted-foreground: 0 0% 45.1%;
  --accent: 0 0% 96.1%;
  --accent-foreground: 0 0% 9%;
  --destructive: 0 84.2% 60.2%;
  --destructive-foreground: 0 0% 98%;
  --border: 0 0% 89.8%;
  --input: 0 0% 89.8%;
  --ring: 0 0% 3.9%;

  /* Role colors (from tree-visualization-demo.html) */
  --role-user: 0 0% 35%;           /* #595757 深灰色 */
  --role-assistant: 201 32% 49%;    /* #5785A3 藍灰色 */
  --role-system: 206 30% 61%;       /* #7B9FBF 淺藍色 */
  --role-tool: 120 40% 49%;         /* #57A357 綠色 */
  --role-checkpoint: 47 58% 58%;    /* #D4C257 黃色 */
  --role-error: 0 55% 50%;          /* #C23C3C 紅色 */
}
```

### Using Colors with Opacity

Tailwind v4 allows opacity modifiers with CSS variables:

```tsx
// Background with 8% opacity
className="bg-[hsl(var(--role-user)/0.08)]"

// Border with 25% opacity
className="border-[hsl(var(--role-user)/0.25)]"

// Hover border with 40% opacity
className="hover:border-[hsl(var(--role-user)/0.4)]"

// Full opacity text
className="text-[hsl(var(--role-user))]"
```

### Component Patterns

#### 1. Use shadcn/ui Components

**Don't** repeat long className strings:
```tsx
// ❌ Bad
<button className="inline-flex items-center justify-center rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 border border-input bg-background hover:bg-accent hover:text-accent-foreground h-9 px-3">
  Click me
</button>
```

**Do** use the Button component:
```tsx
// ✅ Good
import { Button } from "@/components/ui/button";

<Button variant="outline" size="sm">
  Click me
</Button>
```

#### 2. Focus States and Accessibility

Always provide clear focus states and accessibility features:

```tsx
// Focus ring with smooth transitions
<div className={cn(
  "rounded-lg border transition-all",
  isFocused 
    ? "border-ring ring-2 ring-ring/20 shadow-sm"
    : "border-input"
)}>
  {/* Content */}
</div>

// Disabled states
<Button disabled={!canSubmit}>
  Submit
</Button>
```

#### 3. Sticky Headers and Footers

Use glassmorphism for sticky elements:

```tsx
<header className="sticky top-0 z-10 border-b border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
  {/* Header content */}
</header>

<footer className="sticky bottom-0 z-10 border-t border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
  {/* Footer content */}
</footer>
```

#### 4. Visual Hierarchy

Follow Tailwind's typography scale:

```tsx
// Page title
<h1 className="text-2xl font-bold tracking-tight text-foreground">
  Page Title
</h1>

// Section title
<h2 className="text-xl font-semibold text-foreground">
  Section Title
</h2>

// Description
<p className="text-sm text-muted-foreground">
  Helpful description text
</p>

// Body text
<p className="text-sm text-foreground">
  Regular body text
</p>
```

#### 5. Loading States

Provide clear feedback during async operations:

```tsx
import { Loader2 } from "lucide-react";

// Inline loading indicator
{isLoading && (
  <div className="flex items-center text-xs text-muted-foreground">
    <Loader2 className="mr-1 h-3 w-3 animate-spin" />
    Loading...
  </div>
)}

// Button with loading state
<Button disabled={isLoading}>
  {isLoading ? (
    <>
      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
      Sending...
    </>
  ) : (
    <>
      <Send className="mr-2 h-4 w-4" />
      Send
    </>
  )}
</Button>
```

#### 6. Icon Usage

**IMPORTANT:** Always use **lucide-react** for consistent icons across the application. Never use emoji (💬🔬⚡) or other icon libraries.

```tsx
import { Plus, Send, Check, X, Brain, Wrench } from "lucide-react";

// Icon with text
<Button>
  <Plus className="mr-2 h-4 w-4" />
  New Item
</Button>

// Icon only button
<Button size="icon">
  <Send className="h-4 w-4" />
</Button>

// Custom colored icon
<Brain className="w-3.5 h-3.5 text-purple-600" />

// Icon in select/dropdown (positioned absolutely)
const SelectedIcon = MessageCircle;
<div className="relative">
  <SelectedIcon className="w-4 h-4 text-muted-foreground absolute left-3 pointer-events-none" />
  <select className="pl-9">...</select>
</div>
```

**Common Icons:**
- `MessageCircle` - General/chat
- `Code` - Programming/coding
- `FlaskConical` - Research/science
- `Zap` - Quick/fast
- `Brain` - Thinking/reasoning
- `Wrench` - Tools
- `Send` - Submit/send
- `Loader2` - Loading (with animate-spin)
- `Plus` - Add/create
- `X` - Close/remove
- `Check` - Success/confirm
- `CheckCircle`, `XCircle` - Status indicators

### Layout Patterns

#### Full-Screen Chat Interface

```tsx
<div className="flex flex-col h-screen bg-background">
  {/* Sticky header */}
  <header className="sticky top-0 z-10 border-b border-border bg-background/95 backdrop-blur">
    {/* Header content */}
  </header>

  {/* Scrollable content */}
  <main className="flex-1 overflow-y-auto p-4">
    <div className="max-w-4xl mx-auto">
      {/* Main content */}
    </div>
  </main>

  {/* Sticky footer/input */}
  <footer className="sticky bottom-0 z-10 border-t border-border bg-background/95 backdrop-blur">
    {/* Footer content */}
  </footer>
</div>
```

#### Centered Content Container

```tsx
<div className="max-w-4xl mx-auto px-4 py-4">
  {/* Content is centered and has consistent max-width */}
</div>
```

## Component Guidelines

### Message Cards

Message cards use role-based colors with opacity modifiers:

```tsx
const roleStyles = {
  user: "bg-[hsl(var(--role-user)/0.08)] border-[hsl(var(--role-user)/0.25)]",
  assistant: "bg-[hsl(var(--role-assistant)/0.08)] border-[hsl(var(--role-assistant)/0.25)]",
  system: "bg-[hsl(var(--role-system)/0.08)] border-[hsl(var(--role-system)/0.25)]",
};
```

### Auto-Resizing Textarea

```tsx
const textareaRef = useRef<HTMLTextAreaElement>(null);

useEffect(() => {
  const textarea = textareaRef.current;
  if (textarea) {
    textarea.style.height = "auto";
    textarea.style.height = `${textarea.scrollHeight}px`;
  }
}, [message]);

<textarea
  ref={textareaRef}
  rows={1}
  className="resize-none min-h-[36px] max-h-[200px] overflow-y-auto"
/>
```

### Keyboard Shortcuts

```tsx
const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
  // Ctrl+Enter or Cmd+Enter to send
  if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
    e.preventDefault();
    handleSend();
  }
};
```

### Type Imports

Always use `import type` for TypeScript types to avoid runtime imports:

```tsx
// ✅ Good
import { MessageCard } from "./MessageCard";
import type { MessageCardProps } from "./MessageCard";

// ❌ Bad
import { MessageCard, MessageCardProps } from "./MessageCard";
```

## Tailwind CSS v4 Setup

**Important:** Tailwind v4 uses a CSS-first approach. In `src/index.css`:

```css
/* MUST be in this order */
@import "tailwindcss";
@config "../tailwind.config.js";

/* Then custom CSS */
:root {
  /* CSS variables */
}
```

**Do NOT** mix old `@tailwind` directives with v4 `@import`:

```css
/* ❌ Bad - Don't use these in v4 */
@tailwind base;
@tailwind components;
@tailwind utilities;

/* ✅ Good - Use this */
@import "tailwindcss";
```

## Common Utilities

### `cn()` - Conditional Classes

Use the `cn()` utility from `lib/utils.ts` for conditional class merging:

```tsx
import { cn } from "@/lib/utils";

<div className={cn(
  "base-class",
  isActive && "active-class",
  variant === "primary" ? "primary-class" : "secondary-class",
  className // Allow external className override
)}>
  {/* Content */}
</div>
```

## Development Workflow

### Running Development Server

```bash
# Automated (recommended)
python develop.py start      # Start both frontend + backend
python develop.py restart    # Restart backend only
python develop.py stop       # Stop both

# Manual
cargo run --features dev-server -- serve  # Backend with CORS (port 3000)
cd web && npm run dev                      # Frontend with HMR (port 5173)
```

### Building for Production

```bash
cargo build --release  # Auto-builds frontend via build.rs
```

The `build.rs` script automatically:
1. Detects release builds
2. Runs `npm install` if needed
3. Runs `npm run build` to compile frontend
4. Embeds `web/dist/` into binary via `rust-embed`

### Adding Dependencies

```bash
# Frontend dependencies
cd web && npm install <package>

# Example
cd web && npm install date-fns
```

## Best Practices

### 1. Component Organization
- One component per file
- Export types alongside components
- Use `index.ts` for barrel exports when needed

### 2. Styling
- Use Tailwind utilities first
- Extract repeated patterns to components
- Use CSS variables for theme colors
- Avoid inline styles

### 3. State Management
- Use React's built-in hooks (useState, useEffect, useContext)
- Lift state to common ancestor when needed
- Consider context for deeply nested prop drilling

### 4. Performance
- Use React.memo() for expensive components
- Avoid unnecessary re-renders
- Use proper dependency arrays in useEffect
- Lazy load routes if needed

### 5. Accessibility
- Use semantic HTML
- Provide alt text for images
- Ensure keyboard navigation
- Use proper ARIA labels
- Maintain color contrast ratios

### 6. TypeScript
- Define proper interfaces for props
- Use strict type checking
- Avoid `any` - use `unknown` if needed
- Export types for reusability

## Common Patterns

### Modal/Dialog Pattern
```tsx
import { useState } from "react";

function Component() {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <>
      <Button onClick={() => setIsOpen(true)}>Open Dialog</Button>
      {isOpen && (
        <Dialog onClose={() => setIsOpen(false)}>
          {/* Dialog content */}
        </Dialog>
      )}
    </>
  );
}
```

### Form Handling
```tsx
function Form() {
  const [formData, setFormData] = useState({ name: "", email: "" });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    // Handle submission
  };

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setFormData(prev => ({
      ...prev,
      [e.target.name]: e.target.value
    }));
  };

  return (
    <form onSubmit={handleSubmit}>
      <input name="name" value={formData.name} onChange={handleChange} />
      <input name="email" value={formData.email} onChange={handleChange} />
      <Button type="submit">Submit</Button>
    </form>
  );
}
```

### Error Boundaries
```tsx
class ErrorBoundary extends React.Component<{children: React.ReactNode}> {
  state = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  render() {
    if (this.state.hasError) {
      return <div>Something went wrong</div>;
    }
    return this.props.children;
  }
}
```

## Troubleshooting

### Vite Not Picking Up Tailwind Changes
- Restart Vite dev server
- Check `@import "tailwindcss"` is first in index.css
- Ensure `@config` path is correct

### Type Errors with Component Exports
- Use `import type` for type-only imports
- Check TypeScript interfaces are properly exported
- Verify file extensions (.tsx vs .ts)

### Styling Not Applied
- Check class names for typos
- Verify CSS variable is defined in index.css
- Ensure Tailwind is scanning the file (check tailwind.config.js content paths)

### Build Errors in Production
- Test production build locally: `npm run build`
- Check console for asset loading errors
- Verify all imports use correct paths

## Resources

- [React Documentation](https://react.dev)
- [Tailwind CSS v4 Documentation](https://tailwindcss.com)
- [shadcn/ui Documentation](https://ui.shadcn.com)
- [Vite Documentation](https://vitejs.dev)
- [lucide-react Icons](https://lucide.dev)

---

**Last Updated:** 2026-01-12


---

## 🤖 LLM Prompt Library

When working with an LLM (like Claude) on this frontend codebase, use these prompts to ensure adherence to our standards:

### 1. Code Review Prompt
```
Review the following React/TypeScript component code against the frontend guide at doc/front-guide.md. Check for:
- Proper use of shadcn/ui components instead of long className strings
- Correct Tailwind v4 CSS variable usage with HSL opacity modifiers
- Focus states and accessibility features
- Proper type imports (import type vs import)
- Visual hierarchy following our typography scale
- Loading state feedback
- Icon usage from lucide-react
- Keyboard shortcut implementation

Provide specific feedback with code examples for any violations.

[Paste component code here]
```

### 2. Component Generation Prompt
```
Create a new React component following the standards in doc/front-guide.md:
- Use TypeScript with proper interface definitions
- Use shadcn/ui Button component (no raw button elements)
- Follow our color palette from CSS variables (--role-*, --background, etc.)
- Include focus states with ring-2 ring-ring/20
- Add loading states with Loader2 from lucide-react
- Use proper visual hierarchy (text-2xl for titles, text-sm for body)
- Include keyboard shortcuts where applicable
- Use import type for TypeScript types
- Add proper accessibility features

Component requirements: [Describe component here]
```

### 3. Refactoring Prompt
```
Refactor this component to follow doc/front-guide.md standards:
1. Replace long className strings with shadcn/ui components
2. Convert hardcoded colors to CSS variables with opacity modifiers
3. Add proper focus states and accessibility
4. Improve visual hierarchy with proper typography classes
5. Add loading/disabled states with clear feedback
6. Extract repeated patterns into reusable components
7. Use import type for type imports

[Paste component code here]
```

### 4. Style Audit Prompt
```
Audit the styling in this component against doc/front-guide.md:
- Are we using CSS variables (hsl(var(--*))) correctly?
- Are opacity modifiers used properly (/0.08, /0.25, /0.4)?
- Is there proper visual hierarchy (font sizes, weights, colors)?
- Are focus states implemented with ring-ring/20?
- Is glassmorphism used correctly for sticky elements (bg-background/95 backdrop-blur)?
- Are spacing utilities consistent with our max-w-4xl containers?
- Are there any hardcoded color values that should use CSS variables?

[Paste component code here]
```

### 5. Accessibility Audit Prompt
```
Review this component's accessibility against doc/front-guide.md standards:
- Semantic HTML usage
- Keyboard navigation support (tab order, shortcuts)
- Focus indicators (ring-2 ring-ring)
- ARIA labels where needed
- Disabled state handling
- Screen reader compatibility
- Color contrast (use our CSS variables)
- Alt text for images/icons

[Paste component code here]
```

### 6. New Feature Planning Prompt
```
I need to implement [feature description]. Based on doc/front-guide.md:
1. What existing components should I reuse?
2. What new components should I create?
3. What layout pattern should I follow (full-screen, centered container, etc.)?
4. What shadcn/ui components are available?
5. How should I handle state management?
6. What accessibility considerations are needed?
7. Are there similar patterns in the existing codebase I should reference?

Provide a step-by-step implementation plan.
```

### 7. Performance Optimization Prompt
```
Analyze this component for performance issues per doc/front-guide.md:
- Should React.memo() be used?
- Are there unnecessary re-renders?
- Are useEffect dependencies correct?
- Should this component be lazy loaded?
- Are there expensive calculations that should be memoized?
- Is the component properly typed to avoid runtime checks?

[Paste component code here]
```

### 8. Migration/Upgrade Prompt
```
Help me migrate this component to follow our current standards in doc/front-guide.md:
- Current state: [Describe current implementation]
- Target: Updated component following all current best practices
- Ensure backward compatibility where needed
- Update all styling to use CSS variables
- Replace any emoji icons with lucide-react
- Add any missing accessibility features

[Paste component code here]
```

### 9. Testing Prompt
```
Based on doc/front-guide.md, help me write tests for this component:
- What user interactions should be tested?
- What accessibility features need testing?
- What loading/error states should be covered?
- What keyboard shortcuts need testing?
- How should I test TypeScript type safety?

Component: [Describe component]
```

### 10. Documentation Prompt
```
Generate documentation for this component following doc/front-guide.md format:
- Component purpose and use cases
- Props interface with descriptions
- Usage examples with proper imports
- Styling customization options
- Accessibility features
- Keyboard shortcuts
- Related components

[Paste component code here]
```

---
