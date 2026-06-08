(function () {
  const PET_CLIP_DURATION_DEFAULT = 6;

  function prepareInlineVideo(video) {
    if (!(video instanceof HTMLVideoElement)) {
      return;
    }
    video.muted = true;
    video.defaultMuted = true;
    video.playsInline = true;
    video.setAttribute("muted", "");
    video.setAttribute("playsinline", "");
    video.setAttribute("webkit-playsinline", "");
  }

  function bindPetVideoClipLoop(video) {
    if (!(video instanceof HTMLVideoElement) || video.dataset.clipBound === "true") {
      return;
    }
    video.dataset.clipBound = "true";
    prepareInlineVideo(video);

    const clipStart = Number(video.dataset.clipStart || 0);
    const clipDuration = Number(video.dataset.clipDuration || PET_CLIP_DURATION_DEFAULT);

    const seekToClipStart = () => {
      video.currentTime = clipStart;
    };

    const keepInClip = () => {
      if (video.currentTime >= clipStart + clipDuration - 0.05) {
        video.currentTime = clipStart;
      }
    };

    video.addEventListener("loadedmetadata", seekToClipStart);
    video.addEventListener("timeupdate", keepInClip);
    video.addEventListener("ended", () => {
      video.currentTime = clipStart;
      video.play().catch(() => {});
    });
  }

  async function playPetVideoClip(video) {
    if (!(video instanceof HTMLVideoElement)) {
      return;
    }

    prepareInlineVideo(video);
    const clipStart = Number(video.dataset.clipStart || 0);

    const attemptPlay = async () => {
      video.currentTime = clipStart;
      try {
        await video.play();
      } catch (_error) {
        if (video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) {
          await new Promise((resolve) => {
            video.addEventListener("loadeddata", resolve, { once: true });
            video.load();
          });
        }
        video.currentTime = clipStart;
        await video.play().catch(() => {});
      }
    };

    if (video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA) {
      await attemptPlay();
      return;
    }

    await new Promise((resolve) => {
      video.addEventListener("loadeddata", resolve, { once: true });
      if (video.networkState === HTMLMediaElement.NETWORK_EMPTY) {
        video.load();
      }
    });
    await attemptPlay();
  }

  function pausePetVideoClip(video) {
    if (video instanceof HTMLVideoElement) {
      video.pause();
    }
  }

  function bindToggleControl(toggle, onActivate) {
    if (!(toggle instanceof HTMLElement)) {
      return;
    }

    let lastActivateAt = 0;

    const handleActivate = (event) => {
      event.preventDefault();
      event.stopPropagation();
      const now = Date.now();
      if (now - lastActivateAt < 350) {
        return;
      }
      lastActivateAt = now;
      onActivate();
    };

    toggle.addEventListener("click", handleActivate);
    toggle.addEventListener("pointerup", (event) => {
      if (event.pointerType === "mouse") {
        return;
      }
      handleActivate(event);
    });
  }

  document.querySelectorAll(".pet-user-video-player, .account-pet-video-player").forEach((video) => {
    bindPetVideoClipLoop(video);
  });

  const stage = document.getElementById("cinder-pet-stage");
  const videoToggle = stage?.querySelector(".cinder-photo-toggle");
  const videoWrap = stage?.querySelector(".pet-user-video-optional");
  const videoPlayer = stage?.querySelector(".pet-user-video-player");

  function petStageName() {
    return stage?.dataset.petName?.trim() || "your cat";
  }

  if (stage && videoToggle && videoWrap) {
    if (videoPlayer instanceof HTMLVideoElement) {
      bindPetVideoClipLoop(videoPlayer);
    }

    bindToggleControl(videoToggle, () => {
      const showVideo = !videoWrap.classList.contains("is-visible");
      const petName = petStageName();
      videoWrap.classList.toggle("is-visible", showVideo);
      videoWrap.hidden = !showVideo;
      videoToggle.setAttribute("aria-pressed", showVideo ? "true" : "false");
      videoToggle.textContent = showVideo ? `Back to ${petName} 🐾` : "Watch my kitty play! 🎬";
      stage.classList.toggle("video-mode", showVideo);

      if (showVideo) {
        window.requestAnimationFrame(() => {
          playPetVideoClip(videoPlayer);
        });
      } else {
        pausePetVideoClip(videoPlayer);
      }
    });
  }

  const accountStage = document.getElementById("account-pet-photo-stage");
  if (!accountStage) {
    return;
  }

  const accountToggle = accountStage.querySelector(".account-pet-photo-toggle");
  const accountVideoWrap = accountStage.querySelector(".account-pet-video-optional");
  const accountVideoPlayer = accountStage.querySelector(".account-pet-video-player");
  const accountCaption = accountStage.querySelector(".account-pet-photo-caption");
  let accountPetName = accountStage.dataset.petName || "Your cat";
  const accountClipLabel = accountStage.dataset.clipLabel || "playing";

  if (!accountToggle || !accountVideoWrap) {
    return;
  }

  if (accountVideoPlayer instanceof HTMLVideoElement) {
    bindPetVideoClipLoop(accountVideoPlayer);
  }

  function setAccountPhotoMode(showVideo) {
    accountVideoWrap.classList.toggle("is-visible", showVideo);
    accountVideoWrap.hidden = !showVideo;
    accountToggle.setAttribute("aria-pressed", showVideo ? "true" : "false");
    accountToggle.setAttribute(
      "aria-label",
      showVideo ? `Show ${accountPetName} profile photo` : `Show ${accountPetName} playing video`,
    );
    accountStage.classList.toggle("video-mode", showVideo);

    if (accountCaption) {
      accountCaption.textContent = showVideo
        ? `${accountPetName} · ${accountClipLabel} clip`
        : `${accountPetName} · tap photo for playing clip`;
    }

    if (showVideo) {
      window.requestAnimationFrame(() => {
        playPetVideoClip(accountVideoPlayer);
      });
    } else {
      pausePetVideoClip(accountVideoPlayer);
    }
  }

  function toggleAccountPhotoMode() {
    setAccountPhotoMode(!accountVideoWrap.classList.contains("is-visible"));
  }

  document.addEventListener("whisker:pet-name-changed", (event) => {
    const nextName = event.detail?.petName?.trim();
    if (!nextName) {
      return;
    }
    accountPetName = nextName;
    const showVideo = accountVideoWrap.classList.contains("is-visible");
    setAccountPhotoMode(showVideo);
  });

  bindToggleControl(accountToggle, toggleAccountPhotoMode);
  accountToggle.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggleAccountPhotoMode();
    }
  });
})();
