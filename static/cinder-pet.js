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

  function applyPetVideoFraming(video) {
    if (!(video instanceof HTMLVideoElement)) {
      return;
    }

    window.whiskerPetVideoFramer?.applyPlaybackFraming?.(video);
  }

  function watchPetVideoFraming(video) {
    if (!(video instanceof HTMLVideoElement)) {
      return;
    }

    const frame =
      video.closest(".pet-video-framed-viewport, .cinder-pet-image-wrap, .account-pet-photo-wrap");
    if (!(frame instanceof HTMLElement)) {
      return;
    }

    if (typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver(() => {
      applyPetVideoFraming(video);
    });
    observer.observe(frame);
  }

  function initPetMediaIn(root) {
    const scope = root instanceof HTMLElement ? root : document;
    scope
      .querySelectorAll(
        ".pet-user-video-player, .account-pet-video-player, .community-cat-video-player"
      )
      .forEach((video) => {
        bindPetVideoClipLoop(video);
        applyPetVideoFraming(video);
        video.addEventListener("loadedmetadata", () => {
          applyPetVideoFraming(video);
        });
        watchPetVideoFraming(video);
      });
  }

  function initCommunityCatCards(root) {
    const scope = root instanceof HTMLElement ? root : document;
    scope.querySelectorAll(".community-cat-card").forEach((card) => {
      if (card.dataset.communityMediaBound === "true") {
        return;
      }
      const media = card.querySelector(".community-cat-media");
      const toggle = card.querySelector(".community-cat-media-toggle");
      if (!(media instanceof HTMLElement) || !(toggle instanceof HTMLElement)) {
        return;
      }
      const videoWrap = media.querySelector(".community-cat-video-optional");
      const videoPlayer = media.querySelector(".community-cat-video-player");
      if (!(videoWrap instanceof HTMLElement)) {
        return;
      }

      card.dataset.communityMediaBound = "true";
      const petName = media.dataset.petName?.trim() || "this kitty";

      if (videoPlayer instanceof HTMLVideoElement) {
        bindPetVideoClipLoop(videoPlayer);
      }

      bindToggleControl(toggle, () => {
        const showVideo = !videoWrap.classList.contains("is-visible");
        videoWrap.classList.toggle("is-visible", showVideo);
        videoWrap.hidden = !showVideo;
        toggle.setAttribute("aria-pressed", showVideo ? "true" : "false");
        toggle.textContent = showVideo
          ? `Back to ${petName} 🐾`
          : `Watch ${petName} play! 🎬`;
        media.classList.toggle("video-mode", showVideo);

        if (showVideo) {
          window.requestAnimationFrame(() => {
            applyPetVideoFraming(videoPlayer);
            playPetVideoClip(videoPlayer);
          });
        } else {
          pausePetVideoClip(videoPlayer);
        }
      });
    });
  }

  function mountCinderPetStage(stage) {
    if (!(stage instanceof HTMLElement) || stage.dataset.cinderBound === "true") {
      return;
    }

    const videoToggle = stage.querySelector(".cinder-photo-toggle");
    const videoWrap = stage.querySelector(".pet-user-video-optional");
    const videoPlayer = stage.querySelector(".pet-user-video-player");
    if (!videoToggle || !videoWrap) {
      return;
    }

    stage.dataset.cinderBound = "true";

    function petStageName() {
      return stage.dataset.petName?.trim() || "your cat";
    }

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
          window.requestAnimationFrame(() => {
            applyPetVideoFraming(videoPlayer);
            playPetVideoClip(videoPlayer);
          });
        });
      } else {
        pausePetVideoClip(videoPlayer);
      }
    });
  }

  function mountPetCinderStages(root) {
    const scope = root instanceof HTMLElement ? root : document;
    scope.querySelectorAll('.pet-cinder-stage[data-cinder-stage="pet"]').forEach((stage) => {
      mountCinderPetStage(stage);
    });
  }

  initPetMediaIn(document);
  mountPetCinderStages(document);
  initCommunityCatCards(document);

  window.whiskerRemountPetShowcase = function whiskerRemountPetShowcase(root) {
    const scope = root instanceof HTMLElement ? root : document;
    initPetMediaIn(scope);
    mountPetCinderStages(scope);
    initCommunityCatCards(scope);
  };

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
        applyPetVideoFraming(accountVideoPlayer);
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
