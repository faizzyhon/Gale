// Gale website: theme toggle, mobile menu, donate links, guide contents highlight, connection strip.

// Your PayPal donation link. Every "Donate" / "Buy me a coffee" button on the site uses this one value.
// Example: "https://www.paypal.com/paypalme/yourname". While it is empty the buttons open the Support page.
const PAYPAL_URL = "";

(function () {
  const root = document.documentElement;
  const store = {
    get: (k) => { try { return localStorage.getItem(k); } catch { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch { /* storage blocked: theme just isn't remembered */ } },
  };
  const saved = store.get("gale-theme");
  if (saved === "dark" || saved === "light") root.dataset.theme = saved;

  document.addEventListener("DOMContentLoaded", () => {
    const themeBtn = document.getElementById("theme");
    if (themeBtn) {
      themeBtn.addEventListener("click", () => {
        const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
        root.dataset.theme = dark ? "light" : "dark";
        store.set("gale-theme", root.dataset.theme);
      });
    }

    const menuBtn = document.getElementById("menu");
    const nav = document.getElementById("nav");
    if (menuBtn && nav) {
      menuBtn.addEventListener("click", () => {
        const open = nav.classList.toggle("open");
        menuBtn.setAttribute("aria-expanded", String(open));
      });
    }

    if (PAYPAL_URL) {
      document.querySelectorAll("[data-donate]").forEach((a) => {
        a.href = PAYPAL_URL;
        a.target = "_blank";
        a.rel = "noopener";
      });
    }

    // Guide: highlight the section being read.
    const tocLinks = [...document.querySelectorAll(".toc a[href^='#']")];
    if (tocLinks.length && "IntersectionObserver" in window) {
      const byId = new Map(tocLinks.map((a) => [a.getAttribute("href").slice(1), a]));
      const io = new IntersectionObserver((entries) => {
        entries.forEach((e) => {
          if (!e.isIntersecting) return;
          tocLinks.forEach((a) => a.classList.remove("active"));
          byId.get(e.target.id)?.classList.add("active");
        });
      }, { rootMargin: "-20% 0px -70% 0px" });
      byId.forEach((_, id) => { const el = document.getElementById(id); if (el) io.observe(el); });
    }

    // Home: eight connections filling at their own pace, like Gale's per-connection view.
    const bars = [...document.querySelectorAll(".strip-bars i")];
    const read = document.getElementById("strip-read");
    if (bars.length && !matchMedia("(prefers-reduced-motion: reduce)").matches) {
      const rate = bars.map(() => 0.06 + Math.random() * 0.05);
      const done = bars.map((b) => parseFloat(getComputedStyle(b).getPropertyValue("--p")) / 100 || 0.3);
      let last = performance.now();
      const tick = (now) => {
        const dt = Math.min(0.1, (now - last) / 1000);
        last = now;
        let all = 0;
        bars.forEach((b, i) => {
          done[i] = Math.min(1, done[i] + rate[i] * dt);
          b.style.setProperty("--p", (done[i] * 100).toFixed(1) + "%");
          all += done[i];
        });
        const pct = Math.floor((all / bars.length) * 100);
        if (read) read.textContent = pct >= 100 ? "Complete · file verified" : `8 connections · ${pct}%`;
        if (pct >= 100) {
          setTimeout(() => { done.fill(0); rate.forEach((_, i) => (rate[i] = 0.06 + Math.random() * 0.05)); last = performance.now(); requestAnimationFrame(tick); }, 2200);
          return;
        }
        requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    }
  });
})();
