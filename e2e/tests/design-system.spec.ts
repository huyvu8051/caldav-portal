import { test, expect, Page } from '@playwright/test'

/**
 * The same non-negotiables webcaldav asserts, against this app's pages.
 *
 * Ported rather than shared: webcaldav's copy is wired to a CalDAV mock, a
 * seeded session and a month grid, none of which exist here. What carries
 * across is every rule and the reason for it. What does not carry across is
 * the scaffolding.
 *
 * Asserted on computed styles, never class names. A class list says what was
 * intended; a computed style says what shipped.
 */

// Every page this app serves without a session. New pages get added here,
// which is the point: a page that is not in this list is a page the design
// system does not hold for -- and for a while this list was just '/', so
// four pages were unguarded.
const PAGES = ['/', '/privacy', '/terms', '/me']

async function visit(page: Page, url: string) {
  await page.goto(url)
  // Tailwind arrives from a CDN and rewrites the sheet after first paint.
  // Asserting before that is asserting against unstyled HTML, which passes
  // every one of these rules and means nothing.
  await page.waitForFunction(
    () => getComputedStyle(document.body).fontSize === '17px',
    undefined,
    { timeout: 10_000 },
  )
}

test.describe('the design system holds', () => {
  test('nothing anywhere has a shadow', async ({ page }) => {
    // "Depth comes only from surface colour change and 1px hairline borders."
    for (const url of PAGES) {
      await visit(page, url)
      const shadowed = await page.evaluate(() =>
        [...document.querySelectorAll('*')]
          .filter((e) => {
            const s = getComputedStyle(e).boxShadow
            return s && s !== 'none'
          })
          .map((e) => `${e.tagName.toLowerCase()}.${(e.className || '').toString().slice(0, 60)}`),
      )
      expect(shadowed, `box-shadow found on ${url}`).toEqual([])
    }
  })

  test('nothing anywhere has a gradient', async ({ page }) => {
    for (const url of PAGES) {
      await visit(page, url)
      const gradients = await page.evaluate(() =>
        [...document.querySelectorAll('*')]
          .filter((e) => getComputedStyle(e).backgroundImage.includes('gradient'))
          .map((e) => e.tagName.toLowerCase()),
      )
      expect(gradients, `gradient found on ${url}`).toEqual([])
    }
  })

  test('no element uses font-weight 500', async ({ page }) => {
    // The ladder is 300/400/600/700. 500 is absent from the token set, so
    // `font-medium` does not resolve -- this catches the raw `[500]` spelling
    // that gets around that.
    for (const url of PAGES) {
      await visit(page, url)
      const fivehundreds = await page.evaluate(() =>
        [...document.querySelectorAll('*')]
          .filter((e) => getComputedStyle(e).fontWeight === '500')
          .map((e) => e.tagName.toLowerCase()),
      )
      expect(fivehundreds, `font-weight 500 on ${url}`).toEqual([])
    }
  })

  test('body text is 17px, not 16', async ({ page }) => {
    await visit(page, '/')
    const size = await page.evaluate(() => getComputedStyle(document.body).fontSize)
    expect(size).toBe('17px')
  })

  test('the type stack is the system font, with no webfont for text', async ({ page }) => {
    await visit(page, '/')
    const family = await page.evaluate(() => getComputedStyle(document.body).fontFamily)
    expect(family).toMatch(/-apple-system|system-ui|BlinkMacSystemFont/)
  })

  test('every tap target is at least 44px tall', async ({ page }) => {
    // A control inside a >=44px <label> is tappable across the whole row, so
    // the row is the target and the box is just the glyph.
    for (const url of PAGES) {
      await visit(page, url)
      const small = await page.evaluate(() => {
        const effective = (e: Element): number => {
          const label = e.closest('label')
          return Math.max(
            e.getBoundingClientRect().height,
            label?.getBoundingClientRect().height ?? 0,
          )
        }
        return [...document.querySelectorAll('button, select, input, a[role="button"]')]
          .filter((e) => {
            const r = e.getBoundingClientRect()
            return r.width > 0 && r.height > 0 && effective(e) < 43.5
          })
          .map(
            (e) =>
              `${e.tagName.toLowerCase()}[${e.getAttribute('data-testid') ?? ''}] h=${Math.round(effective(e))}`,
          )
      })
      expect(small, `tap target under 44px on ${url}`).toEqual([])
    }
  })

  test('interactive colour is one blue and nothing else', async ({ page }) => {
    await visit(page, '/')
    const accents = await page.evaluate(() => {
      const seen = new Set<string>()
      for (const e of document.querySelectorAll('button, a[role="button"]')) {
        const s = getComputedStyle(e)
        for (const c of [s.color, s.backgroundColor, s.borderTopColor]) {
          const m = c.match(/^rgba?\((\d+), (\d+), (\d+)/)
          if (!m) continue
          const [r, g, b] = [+m[1], +m[2], +m[3]]
          const grey = Math.abs(r - g) < 12 && Math.abs(g - b) < 12
          if (!grey) seen.add(`${r},${g},${b}`)
        }
      }
      return [...seen]
    })

    // Hue, not RGB distance. "One accent" is a statement about hue: every
    // browser composites a disabled button's colour differently and chasing
    // those with a distance threshold is chasing renderers. A red delete is
    // 0 degrees, a green success 120, an amber warning 40 -- none land near
    // Action Blue's 210.
    const hue = (r: number, g: number, b: number): number => {
      const [R, G, B] = [r / 255, g / 255, b / 255]
      const max = Math.max(R, G, B)
      const min = Math.min(R, G, B)
      if (max === min) return NaN
      const d = max - min
      const h = max === R ? ((G - B) / d) % 6 : max === G ? (B - R) / d + 2 : (R - G) / d + 4
      return (h * 60 + 360) % 360
    }
    const ACTION_BLUE_HUE = hue(0, 0x66, 0xcc)

    // Guards against the vacuous pass: a page with no coloured control would
    // satisfy the loop below without proving anything.
    expect(accents.length).toBeGreaterThan(0)
    for (const a of accents) {
      const [r, g, b] = a.split(',').map(Number)
      const h = hue(r, g, b)
      expect(
        Math.abs(h - ACTION_BLUE_HUE),
        `rgb(${a}) is hue ${Math.round(h)}, not Action Blue's ${Math.round(ACTION_BLUE_HUE)}`,
      ).toBeLessThan(10)
    }
  })

  test('the radius ladder is 8 / 11 / 18 / pill and nothing between', async ({ page }) => {
    for (const url of PAGES) {
      await visit(page, url)
      const radii = await page.evaluate(() => {
        const ok = new Set<string>()
        for (const e of document.querySelectorAll('*')) {
          const r = getComputedStyle(e).borderTopLeftRadius
          if (r && r !== '0px') ok.add(r)
        }
        return [...ok]
      })
      const allowed = new Set(['8px', '11px', '18px', '9999px', '50%'])
      for (const r of radii) {
        if (r.endsWith('%')) continue
        expect(allowed.has(r), `radius ${r} on ${url} is not on the ladder`).toBe(true)
      }
    }
  })

  test('nothing is set in all caps or wide tracking', async ({ page }) => {
    for (const url of PAGES) {
      await visit(page, url)
      const shouty = await page.evaluate(() =>
        [...document.querySelectorAll('*')]
          .filter((e) => {
            const s = getComputedStyle(e)
            const spaced = parseFloat(s.letterSpacing)
            return s.textTransform === 'uppercase' || (!Number.isNaN(spaced) && spaced > 0.5)
          })
          .map((e) => e.tagName.toLowerCase()),
      )
      expect(shouty, `shouty text on ${url}`).toEqual([])
    }
  })

  test('every select draws its chevron at the same inset', async ({ page }) => {
    await visit(page, '/')
    const count = await page.locator('select').count()
    // Reported as skipped rather than passed. There are no selects until the
    // database picker lands (A7), and a rule asserted against nothing is the
    // kind of green that means nothing -- the same argument
    // tests/conformance/run.py makes in caldav-rs.
    test.skip(count === 0, 'no <select> on any page yet; this bites once A7 lands')

    const insets = await page.evaluate(() =>
      [...document.querySelectorAll('select')].map((e) => {
        const s = getComputedStyle(e)
        return `${s.appearance}|${s.backgroundPosition}|${s.paddingRight}`
      }),
    )
    expect(new Set(insets).size, `selects disagree: ${JSON.stringify([...new Set(insets)])}`).toBe(1)
    expect(insets[0]).toContain('none')
  })
})
