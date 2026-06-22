(function () {
  const MAX_VIDEO_SECONDS = 10;
  const MAX_PHOTOS_PER_POST = 10;
  const OUTPUT_VIDEO_WIDTH = 720;
  const defaultMediaCta = `Tap to pick up to ${MAX_PHOTOS_PER_POST} photos or one video 🐾`;

  function currentPostsView() {
    const params = new URLSearchParams(window.location.search);
    return params.get("posts_view") === "all" ? "all" : "friends";
  }

  document.querySelectorAll("[data-social-posts-view]").forEach((input) => {
    if (input instanceof HTMLInputElement) {
      input.value = currentPostsView();
    }
  });

  function socialViewportSize() {
    const shellWidth = Math.min(340, Math.max(260, window.innerWidth - 40));
    const width = Math.round(shellWidth);
    const height = Math.round(width * 1.25);
    return { width, height };
  }

  function isImageFile(file) {
    if (!(file instanceof File)) {
      return false;
    }
    if (file.type.startsWith("image/")) {
      return true;
    }
    return /\.(jpe?g|png|webp|heic|heif)$/i.test(file.name);
  }

  function isVideoFile(file) {
    if (!(file instanceof File)) {
      return false;
    }
    if (file.type.startsWith("video/")) {
      return true;
    }
    return /\.(mp4|webm|mov|m4v)$/i.test(file.name);
  }

  function drawVideoFrame(ctx, videoEl, framing, canvasW, canvasH, viewportW, viewportH) {
    const ratioX = canvasW / viewportW;
    const ratioY = canvasH / viewportH;
    const scale = framing.scale;
    const drawW = videoEl.videoWidth * scale * ratioX;
    const drawH = videoEl.videoHeight * scale * ratioY;
    const centerX = canvasW / 2 + framing.offsetX * ratioX;
    const centerY = canvasH / 2 + framing.offsetY * ratioY;

    ctx.fillStyle = "#fad6e9";
    ctx.fillRect(0, 0, canvasW, canvasH);
    ctx.drawImage(videoEl, centerX - drawW / 2, centerY - drawH / 2, drawW, drawH);
  }

  function exportSocialVideo(file, framing, clipStart, clipDuration, viewportW, viewportH) {
    return new Promise((resolve, reject) => {
      const videoEl = document.createElement("video");
      const url = URL.createObjectURL(file);
      videoEl.muted = true;
      videoEl.playsInline = true;
      videoEl.setAttribute("playsinline", "");
      videoEl.setAttribute("webkit-playsinline", "");
      videoEl.preload = "auto";
      videoEl.src = url;

      videoEl.addEventListener("error", () => {
        URL.revokeObjectURL(url);
        reject(new Error("video_load_failed"));
      });

      videoEl.addEventListener("loadedmetadata", () => {
        const canvas = document.createElement("canvas");
        const canvasW = OUTPUT_VIDEO_WIDTH;
        const canvasH = Math.round((OUTPUT_VIDEO_WIDTH * viewportH) / viewportW);
        canvas.width = canvasW;
        canvas.height = canvasH;
        const ctx = canvas.getContext("2d");
        if (!ctx) {
          URL.revokeObjectURL(url);
          reject(new Error("canvas_unavailable"));
          return;
        }

        const mimeType = MediaRecorder.isTypeSupported("video/webm;codecs=vp9")
          ? "video/webm;codecs=vp9"
          : MediaRecorder.isTypeSupported("video/webm")
            ? "video/webm"
            : "";
        if (!mimeType || typeof canvas.captureStream !== "function") {
          URL.revokeObjectURL(url);
          resolve(file);
          return;
        }

        const stream = canvas.captureStream(24);
        const recorder = new MediaRecorder(stream, { mimeType });
        const chunks = [];

        recorder.ondataavailable = (event) => {
          if (event.data.size > 0) {
            chunks.push(event.data);
          }
        };

        recorder.onstop = () => {
          URL.revokeObjectURL(url);
          const blob = new Blob(chunks, { type: mimeType.split(";")[0] });
          if (!blob.size) {
            resolve(file);
            return;
          }
          resolve(
            new File([blob], "social-post.webm", {
              type: blob.type,
              lastModified: Date.now(),
            })
          );
        };

        let stopped = false;
        const stopRecording = () => {
          if (stopped) {
            return;
          }
          stopped = true;
          videoEl.pause();
          if (recorder.state === "recording") {
            recorder.stop();
          }
        };

        recorder.onerror = () => {
          stopRecording();
          resolve(file);
        };

        const renderFrame = () => {
          if (stopped) {
            return;
          }
          drawVideoFrame(ctx, videoEl, framing, canvasW, canvasH, viewportW, viewportH);
          if (videoEl.currentTime >= clipStart + clipDuration - 0.05) {
            stopRecording();
            return;
          }
          requestAnimationFrame(renderFrame);
        };

        videoEl.currentTime = clipStart;
        videoEl.addEventListener(
          "seeked",
          () => {
            try {
              recorder.start(200);
              videoEl.play().catch(() => {
                stopRecording();
              });
              renderFrame();
            } catch (_error) {
              URL.revokeObjectURL(url);
              resolve(file);
            }
          },
          { once: true }
        );
      });
    });
  }

  function formatClock(seconds) {
    const total = Math.max(0, Math.floor(seconds));
    const mins = Math.floor(total / 60);
    const secs = total % 60;
    return `${mins}:${String(secs).padStart(2, "0")}`;
  }

  function initSocialPostForm(form) {
    if (!(form instanceof HTMLFormElement)) {
      return;
    }

    const mediaInput = form.querySelector(".social-post-media-input");
    const mediaCta = form.querySelector(".social-post-media-cta");
    const durationInput = form.querySelector('input[name="video_duration"]');
    const submitButton = form.querySelector('button[type="submit"]');
    const previewRoot = form.querySelector(".social-post-media-preview");
    const previewShell = form.querySelector("[data-social-preview-shell]");

    if (!(mediaInput instanceof HTMLInputElement) || !(previewRoot instanceof HTMLElement)) {
      return;
    }

    const mediaInputId = mediaInput.id;
    const previewId = previewRoot.id;

    function revealPreview() {
      form.closest("details")?.setAttribute("open", "");
      if (previewShell instanceof HTMLElement) {
        previewShell.hidden = false;
        previewShell.classList.add("is-active");
      }
      previewRoot.hidden = false;
      previewRoot.classList.add("is-active");

      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          (previewShell || previewRoot).scrollIntoView({
            behavior: "smooth",
            block: "nearest",
          });
        });
      });
    }

    function hidePreviewShell() {
      if (previewShell instanceof HTMLElement) {
        previewShell.hidden = true;
        previewShell.classList.remove("is-active");
      }
      previewRoot.hidden = true;
      previewRoot.classList.remove("is-active");
    }

    function currentViewport() {
      return socialViewportSize();
    }

    function bindPhotoFramer() {
      const { width, height } = currentViewport();
      window.whiskerPetPhotoFramer?.bind?.(mediaInputId, previewId, {
        viewportWidth: width,
        viewportHeight: height,
        outputWidth: 1080,
        outputHeight: 1350,
        circular: false,
        hint: "Drag and zoom to crop your photo before posting.",
        exportFileName: "social-post.jpg",
        manual: true,
        skipFormSubmit: true,
        onReady: revealPreview,
      });
    }

    bindPhotoFramer();

    let videoState = null;
    let photoPrepared = false;
    let videoPrepared = false;
    let multiPhotoFiles = null;

    function resetVideoEditor() {
      if (videoState?.previewUrl) {
        URL.revokeObjectURL(videoState.previewUrl);
      }
      videoState = null;
      videoPrepared = false;
    }

    function resetAllEditors() {
      resetVideoEditor();
      window.whiskerPetPhotoFramer?.reset?.(mediaInputId);
      photoPrepared = false;
      multiPhotoFiles = null;
      previewRoot.innerHTML = "";
      if (!window.whiskerPetPhotoFramer?.hasImage?.(mediaInputId)) {
        hidePreviewShell();
      }
    }

    function selectedImageFiles() {
      const files = mediaInput.files ? Array.from(mediaInput.files) : [];
      return files.filter((file) => isImageFile(file));
    }

    function showMultiPhotoPreview(files) {
      resetVideoEditor();
      window.whiskerPetPhotoFramer?.reset?.(mediaInputId);
      multiPhotoFiles = files;
      revealPreview();
      previewRoot.innerHTML = `
        <div class="social-post-multi-photo-preview">
          <p class="field-hint">${files.length} photo${files.length === 1 ? "" : "s"} selected · up to ${MAX_PHOTOS_PER_POST} allowed</p>
          <div class="social-post-multi-photo-grid">
            ${files
              .map((file, index) => {
                const url = URL.createObjectURL(file);
                return `<img src="${url}" alt="Photo ${index + 1}" class="social-post-multi-photo-thumb" loading="lazy" />`;
              })
              .join("")}
          </div>
        </div>
      `;
    }

    function setMediaCta(files) {
      if (!(mediaCta instanceof HTMLElement)) {
        return;
      }
      if (!files || files.length === 0) {
        mediaCta.textContent = defaultMediaCta;
        return;
      }
      if (files.length === 1) {
        const file = files[0];
        const kind = isVideoFile(file) ? "🎬" : "📸";
        mediaCta.textContent = `${kind} ${file.name}`;
        return;
      }
      mediaCta.textContent = `📸 ${files.length} photos selected`;
    }

    function setupVideoEditor(file) {
      resetVideoEditor();
      window.whiskerPetPhotoFramer?.reset?.(mediaInputId);

      const { width, height } = currentViewport();
      const previewUrl = URL.createObjectURL(file);
      revealPreview();
      previewRoot.innerHTML = `
        <div class="social-post-video-editor pet-video-trim-editor">
          <p class="pet-video-trim-hint">Preview your clip, drag to reposition, zoom to crop, then post.</p>
          <div class="social-post-video-frame pet-video-trim-frame pet-video-framer-stage" data-video-framer-stage style="width:${width}px;height:${height}px;">
            <video class="social-post-video-preview pet-video-trim-preview pet-video-framer-video" muted playsinline webkit-playsinline preload="metadata"></video>
          </div>
          <label class="pet-video-framer-zoom-label">Zoom
            <input type="range" class="pet-video-framer-zoom social-post-video-zoom" min="0" max="3" step="0.01" value="1" />
          </label>
          <div class="social-post-video-trim-controls" hidden>
            <label>Clip start</label>
            <input class="social-post-video-trim-start" type="range" min="0" max="0" step="0.1" value="0" />
            <output class="pet-video-clip-label social-post-video-trim-label">0:00 – 0:10 (10.0s)</output>
          </div>
        </div>
      `;

      const videoEl = previewRoot.querySelector(".social-post-video-preview");
      const stageEl = previewRoot.querySelector("[data-video-framer-stage]");
      const zoomEl = previewRoot.querySelector(".social-post-video-zoom");
      const trimControls = previewRoot.querySelector(".social-post-video-trim-controls");
      const trimStartEl = previewRoot.querySelector(".social-post-video-trim-start");
      const trimLabel = previewRoot.querySelector(".social-post-video-trim-label");

      if (
        !(videoEl instanceof HTMLVideoElement) ||
        !(stageEl instanceof HTMLElement) ||
        !(zoomEl instanceof HTMLInputElement)
      ) {
        URL.revokeObjectURL(previewUrl);
        return;
      }

      videoState = {
        file,
        previewUrl,
        videoEl,
        duration: 0,
        clipStart: 0,
        clipDuration: MAX_VIDEO_SECONDS,
        framingController: null,
        viewportW: width,
        viewportH: height,
      };

      videoEl.src = previewUrl;

      videoEl.addEventListener("loadedmetadata", () => {
        if (!videoState) {
          return;
        }

        const duration = Number.isFinite(videoEl.duration) ? videoEl.duration : 0;
        if (duration <= 0) {
          window.alert("Could not read that video. Please try another file.");
          mediaInput.value = "";
          resetAllEditors();
          setMediaCta([]);
          return;
        }

        videoState.duration = duration;
        videoState.clipDuration = Math.min(MAX_VIDEO_SECONDS, duration);
        videoState.clipStart = 0;

        const attachFraming = () => {
          if (!videoState) {
            return;
          }
          videoState.framingController = window.whiskerPetVideoFramer?.attachEditor?.({
            videoEl,
            stageEl,
            zoomEl,
            onUpdate: () => {},
          });
        };

        requestAnimationFrame(() => {
          requestAnimationFrame(attachFraming);
        });

        if (duration > MAX_VIDEO_SECONDS && trimControls instanceof HTMLElement) {
          trimControls.hidden = false;
          if (trimStartEl instanceof HTMLInputElement) {
            const maxStart = Math.max(0, duration - MAX_VIDEO_SECONDS);
            trimStartEl.max = String(maxStart);
            trimStartEl.step = "0.1";
            trimStartEl.value = "0";
            trimStartEl.addEventListener("input", () => {
              if (!videoState) {
                return;
              }
              videoState.clipStart = Number.parseFloat(trimStartEl.value) || 0;
              videoState.clipDuration = MAX_VIDEO_SECONDS;
              syncTrimLabel();
              videoEl.currentTime = videoState.clipStart;
            });
          }
        }

        syncTrimLabel();
        videoEl.play().catch(() => {});
        revealPreview();
      });

      videoEl.addEventListener("timeupdate", () => {
        if (!videoState) {
          return;
        }
        const end = videoState.clipStart + videoState.clipDuration;
        if (videoEl.currentTime >= end) {
          videoEl.currentTime = videoState.clipStart;
        }
      });

      function syncTrimLabel() {
        if (!(trimLabel instanceof HTMLOutputElement) || !videoState) {
          return;
        }
        const start = videoState.clipStart;
        const end = start + videoState.clipDuration;
        trimLabel.textContent = `${formatClock(start)} – ${formatClock(end)} (${videoState.clipDuration.toFixed(1)}s)`;
      }
    }

    mediaInput.addEventListener("change", () => {
      const files = mediaInput.files ? Array.from(mediaInput.files) : [];
      photoPrepared = false;
      videoPrepared = false;
      multiPhotoFiles = null;

      if (files.length === 0) {
        resetAllEditors();
        setMediaCta([]);
        return;
      }

      const videos = files.filter((file) => isVideoFile(file));
      const images = files.filter((file) => isImageFile(file));

      if (videos.length > 0 && (images.length > 0 || files.length > 1)) {
        window.alert("Choose up to 10 photos or one video, not both.");
        mediaInput.value = "";
        resetAllEditors();
        setMediaCta([]);
        return;
      }

      if (videos.length > 1) {
        window.alert("Posts can include only one video.");
        mediaInput.value = "";
        resetAllEditors();
        setMediaCta([]);
        return;
      }

      if (images.length > MAX_PHOTOS_PER_POST) {
        window.alert(`Posts can include up to ${MAX_PHOTOS_PER_POST} photos.`);
        mediaInput.value = "";
        resetAllEditors();
        setMediaCta([]);
        return;
      }

      if (videos.length === 1) {
        setMediaCta([videos[0]]);
        bindPhotoFramer();
        setupVideoEditor(videos[0]);
        return;
      }

      if (images.length > 1) {
        setMediaCta(images);
        showMultiPhotoPreview(images);
        return;
      }

      const file = files[0];
      setMediaCta([file]);
      bindPhotoFramer();

      if (isImageFile(file)) {
        resetVideoEditor();
        window.whiskerPetPhotoFramer?.restore?.(mediaInputId, file);
        return;
      }

      window.alert("Please choose a photo or video file.");
      mediaInput.value = "";
      resetAllEditors();
      setMediaCta([]);
    });

    form.addEventListener("submit", async (event) => {
      const files = mediaInput.files ? Array.from(mediaInput.files) : [];
      if (files.length === 0) {
        return;
      }

      if (photoPrepared || videoPrepared) {
        return;
      }

      event.preventDefault();
      if (submitButton instanceof HTMLButtonElement) {
        submitButton.disabled = true;
      }

      try {
        if (multiPhotoFiles && multiPhotoFiles.length > 1) {
          const transfer = new DataTransfer();
          multiPhotoFiles.forEach((file) => transfer.items.add(file));
          mediaInput.files = transfer.files;
          if (durationInput instanceof HTMLInputElement) {
            durationInput.value = "";
          }
          photoPrepared = true;
          form.requestSubmit();
          return;
        }

        const file = files[0];
        if (isImageFile(file)) {
          if (!window.whiskerPetPhotoFramer?.hasImage?.(mediaInputId)) {
            window.alert("Please wait for your photo preview to load.");
            return;
          }
          const cropped = await window.whiskerPetPhotoFramer.exportCroppedFile(mediaInputId);
          const transfer = new DataTransfer();
          transfer.items.add(cropped);
          mediaInput.files = transfer.files;
          if (durationInput instanceof HTMLInputElement) {
            durationInput.value = "";
          }
          photoPrepared = true;
          form.requestSubmit();
          return;
        }

        if (!isVideoFile(file) || !videoState) {
          window.alert("Please wait for your video preview to load.");
          return;
        }

        if (videoState.duration > MAX_VIDEO_SECONDS + 0.05) {
          const maxStart = Math.max(0, videoState.duration - MAX_VIDEO_SECONDS);
          if (videoState.clipStart > maxStart + 0.05) {
            window.alert(`Choose a clip start within the first ${maxStart.toFixed(1)} seconds.`);
            return;
          }
        } else if (videoState.duration > MAX_VIDEO_SECONDS) {
          window.alert(`Please choose a video that is ${MAX_VIDEO_SECONDS} seconds or shorter.`);
          return;
        }

        const framing =
          videoState.framingController?.getState?.() ?? {
            scale: 1,
            offsetX: 0,
            offsetY: 0,
          };

        const exported = await exportSocialVideo(
          file,
          framing,
          videoState.clipStart,
          videoState.clipDuration,
          videoState.viewportW,
          videoState.viewportH
        );

        const transfer = new DataTransfer();
        transfer.items.add(exported);
        mediaInput.files = transfer.files;

        if (durationInput instanceof HTMLInputElement) {
          durationInput.value = String(
            Math.min(videoState.clipDuration, MAX_VIDEO_SECONDS).toFixed(1)
          );
        }

        videoPrepared = true;
        form.requestSubmit();
      } catch (_error) {
        window.alert("Could not prepare that media for posting. Please try again.");
      } finally {
        if (submitButton instanceof HTMLButtonElement) {
          submitButton.disabled = false;
        }
      }
    });
  }

  document.querySelectorAll(".social-post-form").forEach((form) => {
    initSocialPostForm(form);
  });
})();
