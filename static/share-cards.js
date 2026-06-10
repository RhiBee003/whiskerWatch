(function () {
  const modal = document.getElementById("share-card-modal");
  const previewRoot = document.getElementById("share-card-preview");
  const copyBtn = document.getElementById("share-card-copy");
  const nativeBtn = document.getElementById("share-card-native");
  const tweetBtn = document.getElementById("share-card-tweet");
  const instagramBtn = document.getElementById("share-card-instagram");
  const saveImageBtn = document.getElementById("share-card-save-image");
  const closeBtn = document.getElementById("share-card-close");
  const dismissBtn = document.getElementById("share-card-dismiss");

  let activeShareCard = null;

  function escapeHtml(value) {
    return String(value)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  function showToast(message, isError) {
    if (typeof window.whiskerShowToast === "function") {
      window.whiskerShowToast(message, { error: isError === true });
      return;
    }
    window.alert(message);
  }

  function shareCardDefaults(card) {
    const kind = card?.kind === "streak" ? "streak" : "level";
    const value = Number(card?.value) || 0;
    const petName = (card?.pet_name || "my cat").trim() || "my cat";
    const headline =
      card?.headline ||
      (kind === "streak"
        ? `${value} days of loving ${petName}! 🔥💕`
        : `${petName} leveled up to ${value}! 🐾✨`);
    const subline =
      card?.subline ||
      (kind === "streak"
        ? `${value}-day care streak — premium cat care made easy, one task at a time`
        : `Level ${value} unlocked — earn XP from daily care tasks, track routines & paw points`);
    return { kind, value, petName, headline, subline, url: card?.url || "" };
  }

  const SHARE_WIN_BRAND_FOOTER = `<footer class="share-win-brand">
  <img class="share-win-brand-logo" src="/images/logo.png" alt="" width="52" height="52" decoding="async" aria-hidden="true" />
  <p class="share-win-brand-name">WhiskerWatch</p>
  <p class="share-win-brand-tagline">The all-in-one app for serious cat care — daily routines, paw points &amp; real progress.</p>
  <p class="share-win-brand-cta">Join free at whiskerwatch.com</p>
</footer>`;

  function renderShareWinCard(card, celebrateButton) {
    const { kind, value, petName, headline, subline } = shareCardDefaults(card);
    const kindClass = kind === "streak" ? "share-win-card--streak" : "share-win-card--level";
    const kicker = kind === "streak" ? "Care streak milestone" : "Parent level up";
    const heroLabel = kind === "streak" ? (value === 1 ? "DAY" : "DAYS") : "LEVEL";
    const heroEmoji = kind === "streak" ? "🔥" : "⭐";
    const tagline =
      kind === "streak"
        ? "Daily cat care streak — keep the momentum going!"
        : "Cat parent goals unlocked — care tasks that actually count!";
    const celebrate = celebrateButton
      ? '<button type="button" class="share-win-celebrate-btn" data-share-celebrate>Tap for confetti 🎉</button>'
      : "";

    return `<article class="share-win-card ${kindClass}" data-share-kind="${kind}" data-share-value="${value}">
  <div class="share-win-sparkle-field" aria-hidden="true">
    <span class="share-win-sparkle share-win-sparkle-1">✨</span>
    <span class="share-win-sparkle share-win-sparkle-2">💖</span>
    <span class="share-win-sparkle share-win-sparkle-3">🐾</span>
    <span class="share-win-sparkle share-win-sparkle-4">✨</span>
    <span class="share-win-sparkle share-win-sparkle-5">💕</span>
  </div>
  <div class="share-win-confetti-layer" data-share-confetti aria-hidden="true"></div>
  <p class="share-win-kicker">${kicker}</p>
  <div class="share-win-hero">
    <span class="share-win-hero-emoji" aria-hidden="true">${heroEmoji}</span>
    <span class="share-win-hero-value">${value}</span>
    <span class="share-win-hero-label">${heroLabel}</span>
  </div>
  <p class="share-win-headline">${escapeHtml(headline)}</p>
  <p class="share-win-pet">Celebrating <strong>${escapeHtml(petName)}</strong></p>
  <p class="share-win-tagline">${tagline}</p>
  <p class="share-win-subline">${escapeHtml(subline)}</p>
  ${SHARE_WIN_BRAND_FOOTER}
  ${celebrate}
</article>`;
  }

  function instagramCaption(card) {
    const { kind, headline, subline, url } = shareCardDefaults(card);
    const marketing =
      kind === "streak"
        ? "Keep your care streak going on WhiskerWatch — breed guides, vet records & daily routines in one calm dashboard."
        : "Level up by completing daily care tasks on WhiskerWatch — the app that makes premium cat care easy.";
    return `${headline}\n\n${marketing}\n${subline}\n${url}\n\n#WhiskerWatch #CatParent #CatCare`;
  }

  function burstConfetti(layer) {
    if (!(layer instanceof HTMLElement)) {
      return;
    }
    const colors = ["#ff8fc7", "#ffd4ea", "#ffe566", "#b8e0ff", "#ffffff"];
    const pieces = 28;
    layer.innerHTML = "";
    for (let i = 0; i < pieces; i += 1) {
      const piece = document.createElement("span");
      piece.className = "share-win-confetti-piece";
      piece.style.setProperty("--x", `${(Math.random() - 0.5) * 220}px`);
      piece.style.setProperty("--y", `${-40 - Math.random() * 120}px`);
      piece.style.setProperty("--r", `${Math.random() * 360}deg`);
      piece.style.setProperty("--delay", `${Math.random() * 0.18}s`);
      piece.style.background = colors[i % colors.length];
      layer.appendChild(piece);
    }
    layer.classList.add("is-bursting");
    window.setTimeout(() => {
      layer.classList.remove("is-bursting");
      layer.innerHTML = "";
    }, 1400);
  }

  function bindShareCardInteractions(root) {
    if (!(root instanceof HTMLElement)) {
      return;
    }

    const card = root.querySelector(".share-win-card");
    const confettiLayer = root.querySelector("[data-share-confetti]");
    const celebrateBtn = root.querySelector("[data-share-celebrate]");

    const celebrate = () => burstConfetti(confettiLayer);
    if (celebrateBtn instanceof HTMLButtonElement) {
      celebrateBtn.addEventListener("click", (event) => {
        event.stopPropagation();
        celebrate();
      });
    }
    if (card instanceof HTMLElement) {
      card.addEventListener("click", celebrate);
      card.addEventListener("pointermove", (event) => {
        const rect = card.getBoundingClientRect();
        const x = ((event.clientX - rect.left) / rect.width - 0.5) * 10;
        const y = ((event.clientY - rect.top) / rect.height - 0.5) * -10;
        card.style.setProperty("--tilt-x", `${y}deg`);
        card.style.setProperty("--tilt-y", `${x}deg`);
      });
      card.addEventListener("pointerleave", () => {
        card.style.setProperty("--tilt-x", "0deg");
        card.style.setProperty("--tilt-y", "0deg");
      });
    }
  }

  function loadShareLogo() {
    return new Promise((resolve) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => resolve(null);
      img.src = "/images/logo.png";
    });
  }

  async function buildShareCardImageBlob(card) {
    const { kind, value, petName, headline, subline } = shareCardDefaults(card);
    const logo = await loadShareLogo();
    const canvas = document.createElement("canvas");
    canvas.width = 1080;
    canvas.height = 1350;
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      throw new Error("canvas_unavailable");
    }

    const gradient = ctx.createLinearGradient(0, 0, canvas.width, canvas.height);
    gradient.addColorStop(0, "#ffe8f3");
    gradient.addColorStop(0.45, "#ffd4ea");
    gradient.addColorStop(1, "#fff8fc");
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    ctx.fillStyle = "rgba(255, 255, 255, 0.55)";
    roundRect(ctx, 90, 120, 900, 1110, 48);
    ctx.fill();

    ctx.strokeStyle = "rgba(208, 112, 152, 0.45)";
    ctx.lineWidth = 4;
    roundRect(ctx, 90, 120, 900, 1110, 48);
    ctx.stroke();

    ctx.textAlign = "center";
    ctx.fillStyle = "#b85a82";
    ctx.font = "700 42px system-ui, sans-serif";
    ctx.fillText(kind === "streak" ? "CARE STREAK MILESTONE" : "PARENT LEVEL UP", 540, 220);

    ctx.font = "72px system-ui, sans-serif";
    ctx.fillText(kind === "streak" ? "🔥" : "⭐", 540, 380);

    ctx.fillStyle = kind === "streak" ? "#e8357a" : "#d07098";
    ctx.font = "800 220px system-ui, sans-serif";
    ctx.fillText(String(value), 540, 500);

    ctx.fillStyle = "#8f4f6d";
    ctx.font = "700 52px system-ui, sans-serif";
    ctx.fillText(kind === "streak" ? (value === 1 ? "DAY" : "DAYS") : "LEVEL", 540, 580);

    ctx.fillStyle = "#3a1f2f";
    ctx.font = "700 54px system-ui, sans-serif";
    wrapText(ctx, headline, 540, 700, 820, 64);

    ctx.fillStyle = "#6d4a5d";
    ctx.font = "500 40px system-ui, sans-serif";
    ctx.fillText(`Celebrating ${petName}`, 540, 840);

    ctx.fillStyle = "#8f6278";
    ctx.font = "500 34px system-ui, sans-serif";
    wrapText(ctx, subline, 540, 930, 780, 46);

    const brandTop = 1020;
    ctx.strokeStyle = "rgba(208, 112, 152, 0.28)";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(220, brandTop);
    ctx.lineTo(860, brandTop);
    ctx.stroke();

    if (logo) {
      const logoSize = 88;
      ctx.drawImage(logo, 540 - logoSize / 2, brandTop + 28, logoSize, logoSize);
    }

    ctx.fillStyle = "#d07098";
    ctx.font = "800 40px system-ui, sans-serif";
    ctx.fillText("WhiskerWatch", 540, brandTop + (logo ? 150 : 72));

    ctx.fillStyle = "#6d4a5d";
    ctx.font = "500 30px system-ui, sans-serif";
    wrapText(
      ctx,
      "The all-in-one app for serious cat care — daily routines, paw points & real progress.",
      540,
      brandTop + (logo ? 210 : 132),
      760,
      40
    );

    ctx.fillStyle = "#b85a82";
    ctx.font = "700 28px system-ui, sans-serif";
    ctx.fillText("JOIN FREE AT WHISKERWATCH.COM", 540, brandTop + (logo ? 320 : 242));

    return new Promise((resolve, reject) => {
      canvas.toBlob((blob) => {
        if (blob) {
          resolve(blob);
        } else {
          reject(new Error("blob_failed"));
        }
      }, "image/png");
    });
  }

  function roundRect(ctx, x, y, width, height, radius) {
    ctx.beginPath();
    ctx.moveTo(x + radius, y);
    ctx.arcTo(x + width, y, x + width, y + height, radius);
    ctx.arcTo(x + width, y + height, x, y + height, radius);
    ctx.arcTo(x, y + height, x, y, radius);
    ctx.arcTo(x, y, x + width, y, radius);
    ctx.closePath();
  }

  function wrapText(ctx, text, centerX, startY, maxWidth, lineHeight) {
    const words = String(text).split(/\s+/);
    let line = "";
    let y = startY;
    for (const word of words) {
      const test = line ? `${line} ${word}` : word;
      if (ctx.measureText(test).width > maxWidth && line) {
        ctx.fillText(line, centerX, y);
        line = word;
        y += lineHeight;
      } else {
        line = test;
      }
    }
    if (line) {
      ctx.fillText(line, centerX, y);
    }
  }

  async function copyText(text) {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
    throw new Error("clipboard_unavailable");
  }

  async function shareImageFile(card) {
    const blob = await buildShareCardImageBlob(card);
    const file = new File([blob], "whiskerwatch-win.png", { type: "image/png" });
    if (typeof navigator.share === "function" && navigator.canShare?.({ files: [file] })) {
      await navigator.share({
        title: shareCardDefaults(card).headline,
        text: instagramCaption(card),
        files: [file],
      });
      return "shared";
    }
    return blob;
  }

  async function saveShareCardImage(card) {
    try {
      const result = await shareImageFile(card);
      if (result === "shared") {
        showToast("Opened your share sheet — pick Instagram!");
        return;
      }
      const blob = result;
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = "whiskerwatch-win.png";
      link.click();
      URL.revokeObjectURL(url);
      showToast("Card image saved!");
    } catch (_error) {
      showToast("Could not save the card image. Try again.", true);
    }
  }

  async function shareToInstagram(card) {
    try {
      const caption = instagramCaption(card);
      const result = await shareImageFile(card);
      if (result === "shared") {
        return;
      }
      await copyText(caption);
      showToast("Caption copied! Open Instagram and paste into your story or post.");
      window.open("https://www.instagram.com/", "_blank", "noopener,noreferrer");
    } catch (_error) {
      try {
        await copyText(instagramCaption(card));
        showToast("Caption copied! Paste it into Instagram with your card image.", true);
      } catch (_copyError) {
        showToast("Could not share to Instagram right now.", true);
      }
    }
  }

  function closeShareCardModal() {
    if (modal instanceof HTMLElement) {
      modal.hidden = true;
    }
    activeShareCard = null;
  }

  function openShareCardModal(card) {
    if (!(modal instanceof HTMLElement) || !(previewRoot instanceof HTMLElement) || !card?.url) {
      return;
    }

    activeShareCard = card;
    previewRoot.innerHTML = renderShareWinCard(card, true);
    bindShareCardInteractions(previewRoot);
    burstConfetti(previewRoot.querySelector("[data-share-confetti]"));

    const { headline, url } = shareCardDefaults(card);
    if (tweetBtn instanceof HTMLAnchorElement) {
      tweetBtn.href = `https://twitter.com/intent/tweet?text=${encodeURIComponent(`${headline} ${url}`)}`;
    }
    if (nativeBtn instanceof HTMLButtonElement) {
      nativeBtn.hidden = typeof navigator.share !== "function";
    }

    modal.hidden = false;
  }

  async function copyActiveShareLink() {
    if (!activeShareCard?.url) {
      return;
    }
    try {
      await copyText(activeShareCard.url);
      showToast("Share link copied!");
    } catch (_error) {
      showToast("Could not copy the link. Try again.", true);
    }
  }

  async function nativeShareActiveCard() {
    if (!activeShareCard?.url || typeof navigator.share !== "function") {
      return;
    }
    const defaults = shareCardDefaults(activeShareCard);
    try {
      await navigator.share({
        title: defaults.headline,
        text: defaults.subline,
        url: defaults.url,
      });
    } catch (error) {
      if (error?.name !== "AbortError") {
        showToast("Could not open the share sheet.", true);
      }
    }
  }

  if (copyBtn instanceof HTMLButtonElement) {
    copyBtn.addEventListener("click", copyActiveShareLink);
  }
  if (nativeBtn instanceof HTMLButtonElement) {
    nativeBtn.addEventListener("click", nativeShareActiveCard);
  }
  if (instagramBtn instanceof HTMLButtonElement) {
    instagramBtn.addEventListener("click", () => {
      if (activeShareCard) {
        shareToInstagram(activeShareCard);
      }
    });
  }
  if (saveImageBtn instanceof HTMLButtonElement) {
    saveImageBtn.addEventListener("click", () => {
      if (activeShareCard) {
        saveShareCardImage(activeShareCard);
      }
    });
  }
  if (closeBtn instanceof HTMLButtonElement) {
    closeBtn.addEventListener("click", closeShareCardModal);
  }
  if (dismissBtn instanceof HTMLButtonElement) {
    dismissBtn.addEventListener("click", closeShareCardModal);
  }
  if (modal instanceof HTMLElement) {
    modal.addEventListener("click", (event) => {
      if (event.target === modal) {
        closeShareCardModal();
      }
    });
  }

  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) {
      return;
    }
    const shareBtn = target.closest(".share-streak-btn");
    if (!(shareBtn instanceof HTMLButtonElement)) {
      return;
    }
    openShareCardModal({
      url: shareBtn.dataset.shareUrl || "",
      headline: shareBtn.dataset.shareHeadline || "",
      subline: "Daily cat care on WhiskerWatch",
      kind: shareBtn.dataset.shareKind || "streak",
      value: Number(shareBtn.dataset.shareValue || 0),
      pet_name: shareBtn.dataset.sharePet || "",
    });
  });

  document.querySelectorAll(".share-card-preview-wrap--public").forEach((wrap) => {
    bindShareCardInteractions(wrap);
  });

  window.whiskerOpenShareCard = openShareCardModal;
})();
