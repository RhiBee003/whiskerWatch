(function () {
  const params = new URLSearchParams(window.location.search);
  const comfortModal = document.getElementById("memorial-comfort-modal");
  const clipsModal = document.getElementById("memorial-clips-modal");
  const clipsDoneButton = document.getElementById("memorial-clips-done");

  function showAccountTab() {
    const accountTab = document.querySelector('.dashboard-tab[data-tab="account"]');
    if (accountTab instanceof HTMLButtonElement) {
      accountTab.click();
    }
  }

  function lockBodyScroll() {
    document.body.classList.add("modal-open");
  }

  function unlockBodyScrollIfIdle() {
    const anyOpen =
      (comfortModal instanceof HTMLElement && !comfortModal.hidden) ||
      (clipsModal instanceof HTMLElement && !clipsModal.hidden) ||
      Array.from(document.querySelectorAll(".onboarding-backdrop")).some(
        (element) => element instanceof HTMLElement && !element.hidden
      );
    if (!anyOpen) {
      document.body.classList.remove("modal-open");
    }
  }

  function cleanMemorialClipsUrl() {
    if (!params.has("memorial_clips")) {
      return;
    }
    params.delete("memorial_clips");
    const query = params.toString();
    const nextUrl = window.location.pathname + (query ? `?${query}` : "");
    window.history.replaceState({}, document.title, nextUrl);
  }

  function focusNextEmptyClipInput() {
    if (!(clipsModal instanceof HTMLElement)) {
      return;
    }
    const nextInput = clipsModal.querySelector(
      '.memorial-video-slot-form input[type="file"]:not(:disabled)'
    );
    if (nextInput instanceof HTMLInputElement && !nextInput.files?.length) {
      nextInput.focus();
    }
  }

  function openMemorialClipsModal() {
    if (!(clipsModal instanceof HTMLElement)) {
      return;
    }
    showAccountTab();
    clipsModal.hidden = false;
    lockBodyScroll();
    window.scrollTo(0, 0);
    focusNextEmptyClipInput();
    cleanMemorialClipsUrl();
  }

  function closeMemorialClipsModal() {
    if (!(clipsModal instanceof HTMLElement)) {
      return;
    }
    clipsModal.hidden = true;
    unlockBodyScrollIfIdle();
  }

  if (comfortModal instanceof HTMLElement) {
    comfortModal.hidden = false;
    lockBodyScroll();
  }

  if (clipsDoneButton) {
    clipsDoneButton.addEventListener("click", closeMemorialClipsModal);
  }

  if (clipsModal instanceof HTMLElement) {
    clipsModal.addEventListener("click", (event) => {
      if (event.target === clipsModal) {
        closeMemorialClipsModal();
      }
    });
  }

  if (params.get("memorial_clips") === "1") {
    openMemorialClipsModal();
  }

  document.querySelectorAll(".memorial-video-input").forEach((input) => {
    if (!(input instanceof HTMLInputElement)) {
      return;
    }

    input.addEventListener("change", () => {
      const cta = input
        .closest(".memorial-video-upload")
        ?.querySelector(".memorial-video-upload-cta");
      const file = input.files?.[0];
      if (!(cta instanceof HTMLElement) || !file) {
        return;
      }

      const shortName =
        file.name.length > 22 ? "your sweet memory clip" : file.name.replace(/\.[^.]+$/, "");
      cta.textContent = `${shortName} chosen 🎬`;
    });
  });

  const stage = document.getElementById("memorial-photo-stage");
  const cycleButton = document.getElementById("memorial-photo-cycle");
  if (!(stage instanceof HTMLElement) || !(cycleButton instanceof HTMLButtonElement)) {
    return;
  }

  let videos = [];
  try {
    const raw = stage.dataset.memorialVideos || "[]";
    videos = JSON.parse(raw).filter((url) => typeof url === "string" && url.trim());
  } catch (_error) {
    videos = [];
  }

  const photoImage = stage.querySelector(".memorial-photo-image");
  const videoEl = stage.querySelector(".memorial-photo-video");
  const fallbackPhoto = photoImage instanceof HTMLImageElement ? photoImage.src : "";
  let clipIndex = -1;
  let clipBound = false;

  function bindClipLoop(video) {
    if (!(video instanceof HTMLVideoElement) || video.dataset.memorialClipBound === "true") {
      return;
    }
    video.dataset.memorialClipBound = "true";
    video.muted = true;
    video.playsInline = true;
    video.setAttribute("muted", "");
    video.setAttribute("playsinline", "");
    video.setAttribute("webkit-playsinline", "");

    const clipDuration = 6;
    video.addEventListener("timeupdate", () => {
      if (video.currentTime >= clipDuration - 0.05) {
        video.currentTime = 0;
      }
    });
    video.addEventListener("ended", () => {
      video.currentTime = 0;
      video.play().catch(() => {});
    });
  }

  function showPhoto() {
    if (videoEl instanceof HTMLVideoElement) {
      videoEl.pause();
      videoEl.hidden = true;
    }
    if (photoImage instanceof HTMLImageElement) {
      photoImage.hidden = false;
      if (fallbackPhoto) {
        photoImage.src = fallbackPhoto;
      }
    }
  }

  async function showVideo(url) {
    if (!(videoEl instanceof HTMLVideoElement)) {
      return;
    }
    bindClipLoop(videoEl);
    if (photoImage instanceof HTMLImageElement) {
      photoImage.hidden = true;
    }
    videoEl.hidden = false;
    if (videoEl.src !== url) {
      videoEl.src = url;
      videoEl.load();
    }
    videoEl.currentTime = 0;
    try {
      await videoEl.play();
    } catch (_error) {
      // Autoplay may be blocked until the user clicks again.
    }
  }

  function cycleMemory() {
    if (videos.length === 0) {
      showPhoto();
      return;
    }

    clipIndex = (clipIndex + 1) % videos.length;
    showVideo(videos[clipIndex]);
  }

  cycleButton.addEventListener("click", () => {
    cycleMemory();
  });
})();
