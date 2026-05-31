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
