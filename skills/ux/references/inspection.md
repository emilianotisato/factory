# What to look at, and how to actually check it

Two lists. The **state matrix** decides which screens you capture. The **checks** decide what is wrong in one. Both are mechanical; neither needs taste.

---

## The state matrix

Per changed surface. Skip a row only when it genuinely does not apply, and say which you skipped and why — a skipped row silently is a row nobody looked at.

| State | How to reach it | What proves it right |
|---|---|---|
| **Default** | seeded data, the normal path | the thing works and looks deliberate |
| **Empty** | an account or filter with **zero rows** | says what this is, why it is empty, and what to do next. Not a bare "No results." |
| **First run** | a brand-new account, nothing configured | the difference between empty and never-started is usually a different screen |
| **Loading** | throttle the network, or delay the endpoint | a skeleton or spinner in the shape of the content, not a layout that jumps when data lands |
| **Error** | kill the endpoint, return a 500, submit invalid input | says what failed, whether it is retryable, and gives a way out. An error with no action is a dead end |
| **Partial failure** | one widget's request fails, the page loads | the page survives; the broken part is scoped, not the whole screen |
| **Disabled** | the precondition the control needs, unmet | it is visibly disabled *and* something says why. A disabled button with no explanation is a puzzle |
| **Focus** | keyboard only: `Tab` through the whole surface | every interactive element takes focus, in reading order, with a visible ring. Modals trap it; closing returns it |
| **Hover / active** | pointer over each control | present, and not the only affordance — touch has no hover |
| **Selected / open / expanded** | click into each stateful control | the state is visible without color alone |
| **Long content** | a 200-character name, 500 rows, a long URL | truncates or wraps deliberately. Nothing overlaps, nothing pushes the layout sideways |
| **Narrow viewport** | 375px wide | nothing clipped, nothing horizontally scrolling, tap targets still reachable |
| **Dark mode** | the toggle, if the project has one | every surface, not just the ones someone remembered |
| **Slow / offline** | throttle to 3G, then go offline | the app says something. Silence is the failure |

**Reaching a state honestly is the work.** Empty means real emptiness, not a CSS class toggled in devtools. Error means a real failing request. A screenshot of a state you faked proves the CSS, not the behavior, and it is worse than no screenshot because it reads as proof.

Capture per state: a screenshot and an ARIA snapshot. Name them `<surface>.<state>.png` in the harness's artifacts directory.

---

## The checks

Each is a fact, with a way to measure it. Cite the measurement in the finding.

### Spacing, size and color

- **Off-scale spacing.** Grep the diff for arbitrary values — `p-[13px]`, `margin: 13px`, `gap: 7px` — and compare against the rubric's scale. In a Tailwind project, arbitrary-value brackets in a spacing utility are the tell and are greppable in seconds.
- **Off-token color.** A hex or `rgb()` in a component instead of a token or a theme variable. Same grep.
- **Touch targets under 44×44 CSS px.** Measure in the page: `getBoundingClientRect()` over every button, link and input. Under 44 in either dimension on a touch surface is a finding.
- **Contrast under 4.5:1** for body text, **3:1** for large text and for the boundary of an interactive element. Measure computed foreground against computed background; do not eyeball it. Placeholder text and disabled states are where this fails most often, and disabled text still has to be legible.
- **Text that overflows.** Render the long-content state and check `scrollWidth > clientWidth` on text containers.

### Keyboard and assistive

- **Focus indicator removed.** `outline: none` or `outline: 0` in the diff with no `:focus-visible` replacement. This is the single most common accessibility regression and it is a grep.
- **Icon-only control with no accessible name.** In the ARIA snapshot, any button or link whose name is empty. One query over the tree finds them all.
- **Tab order that does not match reading order**, or a positive `tabindex` anywhere.
- **A modal or drawer that does not trap focus**, or does not return focus to what opened it.
- **State conveyed by color alone** — an "active" row that differs only in hue. Check the ARIA snapshot: if the state is not in the accessibility tree, it is not conveyed.
- **Form inputs with no associated label**, and errors not tied to their field via `aria-describedby`.

### Motion and loading

- **Animation with no `prefers-reduced-motion` path.** Grep for keyframes and transitions added in the diff, then check the media query exists.
- **Layout shift as data arrives.** Compare the loading screenshot against the loaded one: if content moves, the placeholder is the wrong shape.
- **A spinner where a skeleton belongs** — anything that loads regularly and has a known shape.
- **No feedback on a slow action.** Click something that takes over ~400ms and watch: an unchanged button that quietly works is indistinguishable from one that ignored the click.

### Content and flow

- **An empty state that only says nothing is here.** It should name what belongs here and offer the action that creates one.
- **An error message that names an exception, a status code, or an internal system.** It should say what happened in the user's terms and what to do.
- **A destructive action with no confirmation, or one whose confirmation does not say what is lost.**
- **Success with no evidence.** A saved thing the user cannot see is saved.
- **A primary action that is not visually primary**, or two competing primaries on one screen.

### Consistency with what already exists

- **A new component that duplicates an existing one.** Search the component directory before accepting any new one; this is the finding that compounds fastest, because the second copy makes the third look normal.
- **A pattern that differs from its neighbours** — this table paginates, every other table scrolls infinitely — without a stated reason.

---

## Writing the finding

One line. Location, the measured fact, the fix.

> `RuleTable.vue:88` — row gap is 13px; the scale has 12 and 16. Use `gap-3`.

> `IconButton.vue:24` — three icon-only buttons have no accessible name; screen readers announce "button". Add `aria-label`.

> `rules-list.empty.png` — the empty state says "No rules." and offers nothing. Every other empty state in this app names the thing and links to create it.

**No essays, no mockups, no adjectives.** A finding that cannot be stated in one line with a measurement is either a question for the owner or not a finding.
