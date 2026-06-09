(function () {
  const OTHER_VALUE = "__other__";

  function restorePetColorPicker(root) {
    const hidden = root.querySelector('[name="pet_color"]');
    const select = root.querySelector("[data-pet-color-select]");
    const customWrap = root.querySelector("[data-pet-color-custom]");
    const customInput = root.querySelector("[data-pet-color-custom-input]");
    if (
      !(hidden instanceof HTMLInputElement) ||
      !(select instanceof HTMLSelectElement) ||
      !(customWrap instanceof HTMLElement) ||
      !(customInput instanceof HTMLInputElement)
    ) {
      return;
    }

    const saved = hidden.value.trim();
    if (!saved) {
      select.value = "";
      customInput.value = "";
      customWrap.hidden = true;
      return;
    }

    const preset = Array.from(select.options).find(
      (option) => option.value && option.value !== OTHER_VALUE && option.value === saved
    );
    if (preset) {
      select.value = saved;
      customInput.value = "";
      customWrap.hidden = true;
      return;
    }

    select.value = OTHER_VALUE;
    customInput.value = saved;
    customWrap.hidden = false;
  }

  function bindPetColorPicker(root) {
    if (root.dataset.petColorPickerBound === "true") {
      restorePetColorPicker(root);
      return;
    }

    const hidden = root.querySelector('[name="pet_color"]');
    const select = root.querySelector("[data-pet-color-select]");
    const customWrap = root.querySelector("[data-pet-color-custom]");
    const customInput = root.querySelector("[data-pet-color-custom-input]");
    if (
      !(hidden instanceof HTMLInputElement) ||
      !(select instanceof HTMLSelectElement) ||
      !(customWrap instanceof HTMLElement) ||
      !(customInput instanceof HTMLInputElement)
    ) {
      return;
    }

    function syncHiddenValue() {
      const selected = select.value;
      if (selected === OTHER_VALUE) {
        hidden.value = customInput.value.trim();
      } else {
        hidden.value = selected;
      }
      hidden.dispatchEvent(new Event("input", { bubbles: true }));
      hidden.dispatchEvent(new Event("change", { bubbles: true }));
    }

    function toggleCustomField() {
      const showCustom = select.value === OTHER_VALUE;
      customWrap.hidden = !showCustom;
      if (showCustom) {
        customInput.focus();
      }
    }

    select.addEventListener("change", () => {
      toggleCustomField();
      syncHiddenValue();
    });

    customInput.addEventListener("input", syncHiddenValue);

    root.closest("form")?.addEventListener("submit", () => {
      syncHiddenValue();
    });

    root.dataset.petColorPickerBound = "true";
    restorePetColorPicker(root);
  }

  document.querySelectorAll("[data-pet-color-picker]").forEach(bindPetColorPicker);

  window.whiskerRestorePetColorPickers = function restorePetColorPickers() {
    document.querySelectorAll("[data-pet-color-picker]").forEach(restorePetColorPicker);
  };
})();
