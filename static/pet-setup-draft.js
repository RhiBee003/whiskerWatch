(function () {
  const DB_NAME = "whiskerPetSetup";
  const DB_VERSION = 1;
  const STORE = "media";

  const CONFIG = {
    onboarding: {
      storageKey: "whiskerOnboardingDraft",
      formSelector: "#onboarding-modal .onboarding-form",
      photoInputId: "pet_photo",
      videoInputId: "pet_video",
      photoBlobKey: "onboarding-photo",
      videoBlobKey: "onboarding-video",
    },
    add_cat: {
      storageKey: "whiskerAddCatDraft",
      formSelector: "#add-cat-modal .add-cat-onboarding-form",
      photoInputId: "add_cat_photo",
      videoInputId: "add_cat_video",
      photoBlobKey: "add-cat-photo",
      videoBlobKey: "add-cat-video",
    },
  };

  let dbPromise = null;
  let saveTimers = {};
  const dirtyKinds = new Set();
  const restoringKinds = new Set();

  function openDb() {
    if (dbPromise) {
      return dbPromise;
    }

    dbPromise = new Promise((resolve, reject) => {
      const request = indexedDB.open(DB_NAME, DB_VERSION);
      request.onupgradeneeded = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains(STORE)) {
          db.createObjectStore(STORE);
        }
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });

    return dbPromise;
  }

  async function saveBlob(key, blob) {
    const db = await openDb();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE, "readwrite");
      tx.objectStore(STORE).put(blob, key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  }

  async function loadBlob(key) {
    const db = await openDb();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE, "readonly");
      const request = tx.objectStore(STORE).get(key);
      request.onsuccess = () => resolve(request.result ?? null);
      request.onerror = () => reject(request.error);
    });
  }

  async function deleteBlob(key) {
    const db = await openDb();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE, "readwrite");
      tx.objectStore(STORE).delete(key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  }

  function getConfig(kind) {
    return CONFIG[kind] ?? null;
  }

  function getForm(kind) {
    const config = getConfig(kind);
    if (!config) {
      return null;
    }
    return document.querySelector(config.formSelector);
  }

  function collectVaccineRows(form) {
    return Array.from(form.querySelectorAll("#vaccine-rows .vaccine-row")).map((row) => ({
      name: row.querySelector('[name="vaccine_names"]')?.value ?? "",
      date: row.querySelector('[name="vaccine_dates"]')?.value ?? "",
    }));
  }

  function collectFormFields(form) {
    const birthHidden = form.querySelector('input[name="pet_birth_date"]');

    return {
      cat_name:
        form.querySelector("#cat_name")?.value ??
        form.querySelector("#add_cat_name")?.value ??
        "",
      pet_breed: form.querySelector('[name="pet_breed"]')?.value ?? "",
      pet_color: form.querySelector('[name="pet_color"]')?.value ?? "",
      pet_birth_date: birthHidden instanceof HTMLInputElement ? birthHidden.value : "",
      pet_indoor_outdoor:
        form.querySelector('input[name="pet_indoor_outdoor"]:checked')?.value ?? "",
      last_vet_date: form.querySelector("#last_vet_date")?.value ?? "",
      never_been_to_vet: Boolean(form.querySelector("#never_been_to_vet")?.checked),
      pet_vaccines_unknown: Boolean(form.querySelector("#pet_vaccines_unknown")?.checked),
      vaccines: collectVaccineRows(form),
      conditions: form.querySelector("#conditions")?.value ?? "",
      medications: form.querySelector("#medications")?.value ?? "",
      skip_video: Boolean(form.querySelector('[name="skip_video"]')?.checked),
      pet_video_clip_start:
        form.querySelector('[name="pet_video_clip_start"]')?.value ??
        form.querySelector("#pet_video_clip_start")?.value ??
        "0",
      pet_video_clip_duration:
        form.querySelector('[name="pet_video_clip_duration"]')?.value ??
        form.querySelector("#pet_video_clip_duration")?.value ??
        "6",
      video_framing: null,
    };
  }

  function collectVideoFraming(kind) {
    const trimController =
      kind === "add_cat"
        ? window.whiskerAddCatPetVideoTrim
        : window.whiskerOnboardingPetVideoTrim;
    const fromController = trimController?.getFramingState?.();
    if (fromController) {
      return fromController;
    }

    const form = getForm(kind);
    const zoom = Number.parseFloat(form?.querySelector('[name="pet_video_zoom"]')?.value ?? "");
    const offsetX = Number.parseFloat(form?.querySelector('[name="pet_video_offset_x"]')?.value ?? "");
    const offsetY = Number.parseFloat(form?.querySelector('[name="pet_video_offset_y"]')?.value ?? "");
    if (!Number.isFinite(zoom) || zoom <= 0) {
      return null;
    }

    return {
      scale: zoom,
      offsetX: Number.isFinite(offsetX) ? offsetX : 0,
      offsetY: Number.isFinite(offsetY) ? offsetY : 0,
    };
  }

  function applyFormFields(form, draft, options = {}) {
    const { preserveBreed = false } = options;

    const setValue = (selector, value) => {
      const field = form.querySelector(selector);
      if (field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement) {
        field.value = value ?? "";
      }
    };

    setValue("#cat_name", draft.cat_name);
    setValue("#add_cat_name", draft.cat_name);
    if (!preserveBreed) {
      setValue('[name="pet_breed"]', draft.pet_breed);
    }
    setValue('[name="pet_color"]', draft.pet_color);
    setValue("#last_vet_date", draft.last_vet_date);
    setValue("#conditions", draft.conditions);
    setValue("#medications", draft.medications);

    const birthPicker = form.querySelector("[data-birth-date-picker]");
    if (birthPicker instanceof HTMLElement && draft.pet_birth_date) {
      if (typeof window.whiskerSetBirthDatePickerValue === "function") {
        window.whiskerSetBirthDatePickerValue(birthPicker, draft.pet_birth_date);
      } else {
        setValue('input[name="pet_birth_date"]', draft.pet_birth_date);
      }
    } else {
      setValue('input[name="pet_birth_date"]', draft.pet_birth_date);
    }

    if (draft.pet_indoor_outdoor) {
      const lifestyle = form.querySelector(
        `input[name="pet_indoor_outdoor"][value="${draft.pet_indoor_outdoor}"]`
      );
      if (lifestyle instanceof HTMLInputElement) {
        lifestyle.checked = true;
      }
    }

    const neverBeenToVet = form.querySelector("#never_been_to_vet");
    if (neverBeenToVet instanceof HTMLInputElement) {
      neverBeenToVet.checked = Boolean(draft.never_been_to_vet);
    }

    const vaccinesUnknown = form.querySelector("#pet_vaccines_unknown");
    if (vaccinesUnknown instanceof HTMLInputElement) {
      vaccinesUnknown.checked = Boolean(draft.pet_vaccines_unknown);
    }

    const skipVideo = form.querySelector('[name="skip_video"]');
    if (skipVideo instanceof HTMLInputElement) {
      skipVideo.checked = Boolean(draft.skip_video ?? draft.skip_photo);
    }

    const clipStart = form.querySelector('[name="pet_video_clip_start"]');
    if (clipStart instanceof HTMLInputElement && draft.pet_video_clip_start != null) {
      clipStart.value = String(draft.pet_video_clip_start);
    }

    const clipDuration = form.querySelector('[name="pet_video_clip_duration"]');
    if (clipDuration instanceof HTMLInputElement && draft.pet_video_clip_duration != null) {
      clipDuration.value = String(draft.pet_video_clip_duration);
    }

    const kind = kindFromForm(form);
    if (kind === "add_cat" && typeof window.whiskerSyncAddCatPetVideoField === "function") {
      window.whiskerSyncAddCatPetVideoField();
    } else if (typeof window.whiskerSyncPetVideoField === "function") {
      window.whiskerSyncPetVideoField();
    }
    if (typeof window.whiskerSyncLastVetDateField === "function") {
      window.whiskerSyncLastVetDateField();
    }
    if (typeof window.whiskerSyncVaccinesUnknownField === "function") {
      window.whiskerSyncVaccinesUnknownField();
    }
    if (
      !draft.pet_vaccines_unknown &&
      Array.isArray(draft.vaccines) &&
      typeof window.whiskerRestoreOnboardingVaccineRows === "function"
    ) {
      window.whiskerRestoreOnboardingVaccineRows(form, draft.vaccines);
      if (typeof window.whiskerSyncVaccinesUnknownField === "function") {
        window.whiskerSyncVaccinesUnknownField();
      }
    }

    if (typeof window.whiskerRestorePetColorPickers === "function") {
      window.whiskerRestorePetColorPickers();
    }
  }

  function kindFromForm(form) {
    if (form.classList.contains("add-cat-onboarding-form")) {
      return "add_cat";
    }
    return "onboarding";
  }

  function fileFromBlob(blob, fallbackName, fallbackType) {
    const name = blob.name || fallbackName;
    const type = blob.type || fallbackType;
    return new File([blob], name, { type, lastModified: Date.now() });
  }

  function setInputFile(input, file) {
    if (!(input instanceof HTMLInputElement)) {
      return;
    }
    const transfer = new DataTransfer();
    transfer.items.add(file);
    input.files = transfer.files;
    input.dispatchEvent(new Event("change", { bubbles: true }));
  }

  async function saveDraft(kind) {
    const config = getConfig(kind);
    const form = getForm(kind);
    if (!config || !(form instanceof HTMLFormElement)) {
      return;
    }

    const draft = collectFormFields(form);
    draft.photo_framing =
      window.whiskerPetPhotoFramer?.getState?.(config.photoInputId) ?? null;
    draft.video_framing = collectVideoFraming(kind);
    sessionStorage.setItem(config.storageKey, JSON.stringify(draft));

    const photoInput = document.getElementById(config.photoInputId);
    const photoFile = photoInput?.files?.[0];
    if (photoFile) {
      await saveBlob(config.photoBlobKey, photoFile);
    }

    const videoInput = document.getElementById(config.videoInputId);
    const videoFile = videoInput?.files?.[0];
    if (videoFile && !draft.skip_video) {
      await saveBlob(config.videoBlobKey, videoFile);
    } else {
      await deleteBlob(config.videoBlobKey);
    }
  }

  function markDirty(kind) {
    if (!restoringKinds.has(kind)) {
      dirtyKinds.add(kind);
    }
  }

  function resetDirty(kind) {
    dirtyKinds.delete(kind);
  }

  async function restoreDraft(kind, options = {}) {
    const { force = false } = options;
    const config = getConfig(kind);
    const form = getForm(kind);
    if (!config || !(form instanceof HTMLFormElement)) {
      return false;
    }

    if (dirtyKinds.has(kind) && !force) {
      return false;
    }

    const raw = sessionStorage.getItem(config.storageKey);
    if (!raw) {
      return false;
    }

    let draft;
    try {
      draft = JSON.parse(raw);
    } catch (_error) {
      return false;
    }

    restoringKinds.add(kind);
    try {
      applyFormFields(form, draft, options);

      const photoBlob = await loadBlob(config.photoBlobKey);
    if (photoBlob) {
      const photoInput = document.getElementById(config.photoInputId);
      const photoFile = fileFromBlob(photoBlob, "pet-photo.jpg", "image/jpeg");
      if (window.whiskerPetPhotoFramer?.restore) {
        window.whiskerPetPhotoFramer.restore(
          config.photoInputId,
          photoFile,
          draft.photo_framing ?? null
        );
      } else if (photoInput) {
        setInputFile(photoInput, photoFile);
      }
    }

    if (!draft.skip_video) {
      const videoBlob = await loadBlob(config.videoBlobKey);
      if (videoBlob) {
        const videoInput = document.getElementById(config.videoInputId);
        const videoFile = fileFromBlob(videoBlob, "pet-video.mp4", "video/mp4");
        const trimController =
          kind === "add_cat"
            ? window.whiskerAddCatPetVideoTrim
            : window.whiskerOnboardingPetVideoTrim;
        if (trimController?.restoreFromFile) {
          trimController.restoreFromFile(videoFile, {
            clipStart: Number.parseFloat(draft.pet_video_clip_start) || 0,
            clipDuration: Number.parseFloat(draft.pet_video_clip_duration) || 6,
            framing: draft.video_framing ?? null,
          });
        } else if (videoInput) {
          setInputFile(videoInput, videoFile);
        }
      }
    }
    } finally {
      restoringKinds.delete(kind);
    }

    return true;
  }

  async function clearDraft(kind) {
    const config = getConfig(kind);
    if (!config) {
      return;
    }

    sessionStorage.removeItem(config.storageKey);
    await deleteBlob(config.photoBlobKey);
    await deleteBlob(config.videoBlobKey);
    resetDirty(kind);
  }

  function scheduleSave(kind) {
    if (!getConfig(kind)) {
      return;
    }

    clearTimeout(saveTimers[kind]);
    saveTimers[kind] = window.setTimeout(() => {
      saveDraft(kind).catch(() => {});
    }, 250);
  }

  function bindAutosave(kind) {
    const form = getForm(kind);
    if (!(form instanceof HTMLFormElement)) {
      return;
    }

    const handleChange = () => {
      markDirty(kind);
      scheduleSave(kind);
    };
    form.addEventListener("input", handleChange);
    form.addEventListener("change", handleChange);
    window.addEventListener("pagehide", () => {
      saveDraft(kind).catch(() => {});
    });
  }

  window.whiskerPetSetupDraft = {
    saveDraft,
    restoreDraft,
    clearDraft,
    scheduleSave,
    bindAutosave,
    markDirty,
    resetDirty,
  };
})();
