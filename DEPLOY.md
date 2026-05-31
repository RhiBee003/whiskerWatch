# WhiskerWatch deployment

## Stripe payments (PCI DSS / SAQ A)

Card numbers and CVV are **never** sent to WhiskerWatch. Checkout uses [Stripe Checkout](https://stripe.com/docs/payments/checkout) (hosted payment page). This app only stores Stripe session IDs after payment and credits paw points via webhooks or the success redirect.

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

## User accounts and SQLite persistence

Accounts and passwords are stored in **`$DATA_DIR/whiskerwatch.db`** (SQLite). Passwords are hashed with bcrypt before storage.

| Environment | `DATA_DIR` | Notes |
|-------------|------------|-------|
| Local (`cargo run`) | Auto-detected project `data/` folder | Walks up from the working directory to find `Cargo.toml`, so the same database is used even if you run the binary from another directory. Override with `DATA_DIR=/path/to/data`. |
| Render (see `render.yaml`) | `/data` | Requires the **persistent disk** mounted at `/data`. Without it, accounts are wiped on every deploy or restart. |

### Verify persistence locally

```bash
cd whiskerWatch
cargo run
# Sign up at http://127.0.0.1:3000/signup, then stop the server (Ctrl+C) and run cargo run again.
# Log in with the same email and password — it should succeed.
sqlite3 data/whiskerwatch.db "SELECT email FROM users;"
```

On startup the server logs the resolved database path, for example:

`Using data directory: /path/to/whiskerWatch/data (database: .../whiskerwatch.db)`

If login fails after a restart, confirm you are hitting the same database file shown in that log line.

### Render checklist

1. Deploy as a **Web Service** (Rust), not a Static Site — see comments in `render.yaml`.
2. Attach the persistent disk (`whiskerwatch-data` → `/data`) on Starter plan or higher.
3. Set `DATA_DIR=/data` (already in `render.yaml`).
4. After changing disk settings, redeploy once so the mount is active before creating accounts.
