# WhiskerWatch To-Do

## Done recently (verify on deploy)
- [x] Wire password-reset emails through SMTP (`email_delivery`)
- [x] Stop storing plaintext password in login-prefill cookie (email only)
- [x] Load HTML templates via `path_in_project` (cwd-safe)
- [x] Fix push notification icon/badge paths (`sw.js`)
- [x] Auto-login after sign up
- [x] Breed select page by category
- [x] Health tab: vet info + no-pet CTA
- [x] Task completion toast without full-page refresh (AJAX path)

## UI / Styling
- [x] Make download button smaller
- [ ] Replace all home images with Figma assets
- [ ] Grey bottom section: extend with swipe
- [ ] All form boxes: light pink background (not yellow)
- [ ] Change levels font color to a different pink (currently blue)
- [ ] Task completed banner should be pink (not green)

## UX / Behavior
- [ ] Change email (account setting — not implemented yet)
- [ ] Make the pet bigger
- [ ] Fix top of tabs being cut off (My Pet, Paw Points, etc.)

## Deploy
- [ ] Point production hostname at Rust app (`whiskerwatch-dh13` is current Rust; `whiskerwatch.onrender.com` still old React SPA)
- [ ] Confirm GitHub default branch is `main` for Render Blueprint
- [ ] Set `SMTP_*`, Stripe, and VAPID secrets on Render

## Tell Us About Your Cat Page
- [ ] Extend blurry background to the bottom of the screen on mobile
