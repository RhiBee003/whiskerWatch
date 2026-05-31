# Render setup for WhiskerWatch (Rust web service)

**Goal:** Replace the old March 2025 **Static Site** (React SPA at `https://whiskerwatch.onrender.com`) with a **Web Service** running the Rust app from the `main` branch.

**Repo:** https://github.com/RhiBee003/whiskerWatch  
**Blueprint:** `render.yaml` in repo root  
**Target URL:** https://whiskerwatch.onrender.com

---

## What was automated (no Render login required)

| Done | Detail |
|------|--------|
| ✅ | `render.yaml` Blueprint for Rust web service + 1 GB disk at `/data` |
| ✅ | `DEPLOY.md` — Stripe, SQLite persistence, env vars |
| ✅ | Rust app reads `PORT`, `DATA_DIR`, `RENDER_EXTERNAL_URL` / `PUBLIC_APP_URL` |
| ✅ | Code pushed to **`main`** on GitHub |
| ❌ | Render API deploy — **`RENDER_API_KEY` not set** in this environment |
| ❌ | GitHub default branch — still **`cursor/rust-website-bootstrap`** (needs one-time fix below) |
| ❌ | Delete old Static Site / sync Blueprint — **requires Render dashboard login** |

---

## Before you start (verify the problem)

```bash
curl -s https://whiskerwatch.onrender.com | head -15
```

If you see `<div id="root"></div>` and `/assets/index-*.js`, the site is still the **old React Static Site**. Follow the steps below.

After a successful Rust deploy you should **not** see `id="root"`; the homepage is server-rendered HTML from `static/index.html`.

---

## Step 1 — Fix GitHub default branch (one time)

Render Blueprint auto-deploy uses the repo’s **default branch** unless `branch: main` is set in `render.yaml` (it is). Still set default to `main` so PRs and clones match production.

### Option A — GitHub website (fastest)

1. Open https://github.com/RhiBee003/whiskerWatch/settings/branches  
2. Under **Default branch**, switch from `cursor/rust-website-bootstrap` → **`main`**  
3. Confirm

### Option B — GitHub CLI (if logged in)

```bash
gh auth login
gh api repos/RhiBee003/whiskerWatch -X PATCH -f default_branch=main
```

---

## Step 2 — Render dashboard (minimal clicks)

You must be logged in at https://dashboard.render.com as the account that owns `whiskerwatch.onrender.com`.

### 2a. Delete the old Static Site

1. Dashboard → find the **Static Site** serving Whisker Watch (created ~March 2025)  
2. **Settings** → scroll to **Delete Static Site** → confirm  

> **Why:** Render cannot change a Static Site into a Web Service. The subdomain `whiskerwatch` is freed when the old service is deleted.

### 2b. Create from Blueprint

1. Dashboard → **New** → **Blueprint**  
2. Connect GitHub repo **`RhiBee003/whiskerWatch`** if not already connected  
3. Render detects **`render.yaml`** → **Apply**  
4. When prompted for secret env vars, enter (or skip Stripe until later):
   - `STRIPE_SECRET_KEY` — from [Stripe Dashboard → API keys](https://dashboard.stripe.com/test/apikeys) (`sk_test_...` or `sk_live_...`)
   - `STRIPE_WEBHOOK_SECRET` — from Stripe → Webhooks → endpoint `https://whiskerwatch.onrender.com/webhooks/stripe`, event `checkout.session.completed`
   - `STRIPE_PUBLISHABLE_KEY` — optional for current server-side Checkout

5. Wait for the first deploy (Rust build ~5–10 min on Starter)

### 2c. After first deploy

1. **Environment** tab → copy the generated **`ADMIN_PASSWORD`** (Blueprint created it; not in git)  
2. Log in at https://whiskerwatch.onrender.com/login with **`ADMIN_EMAIL`** = `rhibee003@gmail.com` and that password  
3. Confirm **Disks** shows `whiskerwatch-data` mounted at **`/data`**

---

## Step 3 — Stripe webhook (production payments)

1. [Stripe Dashboard → Webhooks](https://dashboard.stripe.com/webhooks) → **Add endpoint**  
2. URL: `https://whiskerwatch.onrender.com/webhooks/stripe`  
3. Event: `checkout.session.completed`  
4. Copy signing secret → Render → **whiskerwatch** → **Environment** → `STRIPE_WEBHOOK_SECRET` → **Save & redeploy**

See `DEPLOY.md` for paw-point packages and test cards.

---

## Optional — API / CLI deploy (after service exists)

Set these locally (never commit):

```bash
export RENDER_API_KEY='rnd_...'   # Render → Account Settings → API Keys
export RENDER_SERVICE_ID='srv_...' # Service → Settings → Service ID
```

Trigger a deploy without opening the dashboard:

```bash
./scripts/render-deploy.sh
```

Or manually:

```bash
curl -s -X POST "https://api.render.com/v1/services/${RENDER_SERVICE_ID}/deploys" \
  -H "Authorization: Bearer ${RENDER_API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{}'
```

### Deploy hook (manual redeploy URL)

After the Web Service exists: **whiskerwatch** → **Settings** → **Deploy Hook** → copy URL.

```bash
curl -X POST 'https://api.render.com/deploy/srv-XXXX?key=YYYY'
```

No API key needed; treat the URL as a secret.

---

## Verify success

```bash
# Should NOT contain id="root"
curl -s https://whiskerwatch.onrender.com | grep -E 'root|Whisker'

# Health check
curl -sI https://whiskerwatch.onrender.com | head -5
```

Sign up a test user, redeploy once from the dashboard, and confirm login still works (SQLite on `/data` disk).

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Still React SPA | Old Static Site not deleted, or DNS/cache; delete Static Site and finish Blueprint |
| Build fails | Check **Logs**; ensure `main` has latest commits |
| Accounts lost on redeploy | Disk missing or `DATA_DIR` not `/data`; Starter plan + disk in `render.yaml` |
| Payments disabled | Set `STRIPE_SECRET_KEY` on Render and redeploy |
| Wrong branch deploying | GitHub default branch + `branch: main` in `render.yaml` |

---

## Files reference

| File | Purpose |
|------|---------|
| `render.yaml` | Render Blueprint (Web Service + disk + env) |
| `DEPLOY.md` | Stripe PCI notes, env var table, local SQLite testing |
| `scripts/render-deploy.sh` | Trigger deploy via Render API when keys are set |
| `scripts/verify-render.sh` | Quick check: React SPA vs Rust site |
