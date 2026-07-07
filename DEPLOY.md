# WhiskerWatch deployment

## Render (production)

**Full click-by-click setup:** see [`RENDER_SETUP.md`](RENDER_SETUP.md).

Quick facts:

- Deploy as a **Web Service** (Rust) via `render.yaml`, **not** a Static Site.
- GitHub branch: **`main`** (set as default branch on GitHub).
- Persistent disk: `/data` → `$DATA_DIR/whiskerwatch.db`.
- Live URL: https://whiskerwatch.onrender.com

Verify after deploy: `./scripts/verify-render.sh`

## Stripe payments (PCI DSS / SAQ A)

Card numbers and CVV are **never** sent to WhiskerWatch. Checkout uses [Stripe Checkout](https://stripe.com/docs/payments/checkout) (hosted payment page). Saved cards live in **Stripe** (Customer + PaymentMethod objects). This app stores only each user's Stripe Customer id (`cus_...`) in their profile JSON in SQLite—never PAN, CVV, or magnetic-stripe data. Paw points are credited via webhooks or the success redirect using Stripe session ids.

Funds settle to **your bank** when you connect a payout account in the [Stripe Dashboard](https://dashboard.stripe.com/settings/payouts).

### Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `STRIPE_SECRET_KEY` | Yes (for payments) | Secret key (`sk_live_...` or `sk_test_...`) from Stripe Dashboard → Developers → API keys |
| `STRIPE_WEBHOOK_SECRET` | Production | Signing secret from Developers → Webhooks → endpoint `https://YOUR_HOST/webhooks/stripe`, event `checkout.session.completed` |
| `STRIPE_PUBLISHABLE_KEY` | No | Not used by server-side Checkout redirect; optional for future client features |
| `PUBLIC_APP_URL` | Recommended | Public site URL, e.g. `https://whiskerwatch.onrender.com` (no trailing slash). On Render, `RENDER_EXTERNAL_URL` is used if unset |

If `STRIPE_SECRET_KEY` is missing, the Account tab shows **Payments not configured** instead of checkout buttons.

### Stripe Dashboard setup

1. Create a [Stripe account](https://dashboard.stripe.com/register).
2. **Connect your bank** for payouts: Settings → Payouts → add bank account (verification may take a few days).
3. Copy **Secret key** into `STRIPE_SECRET_KEY` on Render (or local `.env` for development).
4. Add a **webhook** endpoint:
   - URL: `https://YOUR_HOST/webhooks/stripe`
   - Events: `checkout.session.completed`
   - Copy the signing secret into `STRIPE_WEBHOOK_SECRET`.
5. For local testing, use the [Stripe CLI](https://stripe.com/docs/stripe-cli):  
   `stripe listen --forward-to localhost:3000/webhooks/stripe`

### Paw point packages (USD)

| Points | Price |
|--------|-------|
| 100 | $3.00 |
| 250 | $5.00 |
| 500 | $9.00 |
| 1,000 | $15.00 |
| 5,000 | $50.00 |

Test cards: [Stripe test cards](https://stripe.com/docs/testing#cards) (e.g. `4242 4242 4242 4242`).

### Saved payment methods

- On first paw points checkout, the server creates or links a **Stripe Customer** for the account email and saves `stripe_customer_id` on the user profile.
- Checkout is created with `customer`, `payment_intent_data[setup_future_usage]=off_session`, and `saved_payment_method_options[payment_method_save]=enabled` so Stripe prompts to save the card and attaches it to the Customer.
- Return visits use the same Customer; Checkout shows **saved cards** for one-click payment when available.
- The Account tab lists saved cards by calling Stripe's Payment Methods API—displaying **brand, last4, and expiration** only.
- To remove or manage cards in production, use the [Stripe Customer Portal](https://stripe.com/docs/customer-management/customer-portal) or Dashboard; this app does not store card numbers locally.

## User accounts and SQLite persistence

All app data lives under **`$DATA_DIR`**:

- **`whiskerwatch.db`** (SQLite) — accounts, sessions, forum, feedback, social posts, messages
- **`uploads/`** — pet photos, playing clips, social post media, memorial videos

| Environment | `DATA_DIR` | Notes |
|-------------|------------|-------|
| Local (`cargo run`) | Auto-detected project `data/` folder | Walks up from cwd and the binary path to find `Cargo.toml`, so the same database is used even if you run from another directory. Relative `DATA_DIR=data` is also anchored to the project root when found. Override with an absolute path (e.g. `DATA_DIR=/path/to/data`) if needed. |
| Render Starter (see `render.yaml`) | `/data` | Requires the **Starter** plan and a **persistent disk** mounted at `/data`. Keeps the database and `uploads/` across redeploys. |

Legacy `users.jsonl`, `user_profiles.jsonl`, `contact_messages.jsonl`, and `feedback.jsonl` files in `DATA_DIR` are imported into SQLite automatically on first startup when the matching table is empty.

### Verify persistence locally

```bash
cd whiskerWatch
cargo run
# Sign up, post forum Q&A, submit feedback, then stop the server (Ctrl+C) and run cargo run again.
sqlite3 data/whiskerwatch.db "SELECT email FROM users;"
sqlite3 data/whiskerwatch.db "SELECT title FROM forum_posts;"
sqlite3 data/whiskerwatch.db "SELECT category, message FROM feedback;"
```

On startup the server logs the resolved database path and row counts, for example:

`Using data directory: /path/to/whiskerWatch/data (database: .../whiskerwatch.db)`  
`SQLite contains 3 users, 2 forum posts, 1 forum replies, 4 feedback entries, 0 contact messages`

If data disappears after a restart, confirm you are hitting the same database file shown in that log line.

### Render checklist

1. Deploy as a **Web Service** (Rust), not a Static Site — see comments in `render.yaml`.
2. Attach the persistent disk (`whiskerwatch-data` → `/data`) on Starter plan or higher.
3. Set `DATA_DIR=/data` (already in `render.yaml`).
4. After changing disk settings, redeploy once so the mount is active before creating accounts.
