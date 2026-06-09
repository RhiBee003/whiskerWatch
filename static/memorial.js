(function () {
  const comfortModal = document.getElementById("memorial-comfort-modal");
  if (comfortModal instanceof HTMLElement) {
    comfortModal.hidden = false;
  }

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
