# The rubric — write down what this project already does

`docs/ux-rubric.md`, beside `docs/ideas/` and `decisions.md`. The triad: **ideas** = not yet decided to build, **decisions** = engineering forks taken, **ux-rubric** = what "finished" looks like on screen here. Optional, created on first use; `factory docs preflight` judges a UI project without one.

**Read it out of the code, not out of a style guide.** A rubric that says "use consistent spacing" changes nothing. One that says "the scale is 4/8/12/16/24/32; arbitrary values are a finding" makes every future review objective and every future disagreement short.

## Build it

1. **Find the design system.** The Tailwind or theme config, CSS custom properties, a tokens file, the component library in use (shadcn, Radix, Material, a bespoke `components/ui`), the base stylesheet. What is defined is the answer.
2. **Find the patterns already in use.** Grep for the existing empty states, error components, skeletons, toasts, modals, form-error rendering. **The best-built existing example becomes the reference implementation**, cited by path, so a review can say "match `EmptyState.vue`" instead of describing one.
3. **Find the contradictions.** Two spacing systems, two ways of showing an error, three button variants that mean the same thing. These are the only things worth asking the owner about, and each is one AskUserQuestion with the candidates and their call sites counted: "seven components use A, two use B — standardise on A?"
4. **Ask what the code cannot say**: the copy voice, the motion budget, how much the project cares about dark mode or reduced motion, whether mobile is a real target or an afterthought.
5. **Write it short.** A rubric nobody reads enforces nothing. Every line is checkable and cites where the answer lives.

## Shape

```markdown
# UX rubric

What "finished" looks like in this project. `/ux` checks against this; `/polish` runs that review when a diff touches UI.

**Rules**
- Every line here is checkable. A line that needs interpretation belongs in a conversation, not here.
- A line a machine could enforce should become a lint rule, an arch test or a gate tier instead — then it comes out of this file. Hand it to `/gate`.
- When the owner corrects something a review missed, that correction becomes a line here. That is how this file grows; it is not written once.

## Foundations

- **Spacing scale**: {4 / 8 / 12 / 16 / 24 / 32 — Tailwind defaults, `tailwind.config.ts`}. Arbitrary values (`p-[13px]`) are a finding.
- **Color**: {tokens in `resources/css/app.css`; semantic names only — `bg-surface`, `text-muted`}. A raw hex in a component is a finding.
- **Typography**: {the scale, and which sizes are actually in use}.
- **Radius / shadow / border**: {the allowed set}.
- **Breakpoints**: {sm 640 / md 768 / lg 1024}. **Mobile is {a real target | best-effort}**; the review checks {375px}.
- **Dark mode**: {supported — every surface | not supported}.
- **Component library**: {shadcn-vue in `components/ui`}. **Check for an existing component before adding one.**

## Patterns, with their reference implementation

- **Empty state** — {names what belongs here, one sentence of why it is empty, and the action that creates one}. Reference: `{components/EmptyState.vue}`.
- **Loading** — {skeleton in the shape of the content for lists and cards; spinner only for actions under a second}. Reference: `{components/Skeleton.vue}`.
- **Error** — {what failed in the user's words, whether it is retryable, one action out}. Never an exception or a status code. Reference: `{components/ErrorBanner.vue}`.
- **Form validation** — {inline, under the field, on blur; the first invalid field takes focus on submit}.
- **Destructive confirm** — {names what is lost and how many; the confirm button carries the verb, not "OK"}.
- **Success feedback** — {toast, plus the changed state visible on the page}.
- **Focus** — {`:focus-visible` ring, {2px}, token {ring}}. Removing an outline without replacing it is a finding.
- **Motion** — {≤{200}ms, ease-out, opacity and transform only}. Every animation has a `prefers-reduced-motion` path.

## Copy voice

- {Second person, present tense. Say what happened and what to do next.}
- {No exclamation marks. No "Oops". No blaming the user.}
- {Buttons carry the verb: "Create rule", not "Submit".}
- {Sentence case everywhere, including headings and buttons.}

## Accessibility floor

- {Contrast 4.5:1 body, 3:1 large text and interactive boundaries.}
- {Every interactive element reachable and operable by keyboard, in reading order.}
- {Icon-only controls carry an accessible name.}
- {Touch targets ≥44px on touch surfaces.}
- {State never conveyed by color alone.}

## Deliberately not enforced

- {What the project has decided not to care about, so a review stops raising it. "No RTL support — no RTL locales planned."}

## Enforced mechanically, not here

- {`eslint-plugin-tailwindcss` rejects arbitrary spacing — added {date}.}
- {`IconButtonTest` asserts every icon button has a name — added {date}.}

{This list should grow and this file should shrink. A rule enforced by a machine never needs a reviewer to remember it.}
```

## The loop that makes it worth keeping

The rubric earns its place only if it grows from real corrections. Every time the owner fixes something a review did not catch, ask one question: *would a line here have caught it?* If yes, add the line. If a machine could check that line, hand it to `/gate` as a tier and delete the line.

The end state is a short rubric and a long "enforced mechanically" list. That is the same principle the framework applies everywhere else: encode the lesson in structure, not in more text for someone to remember.
