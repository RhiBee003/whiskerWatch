(function () {
  const petStage = document.querySelector(".cat-home-pet-stage");
  if (!petStage) {
    return;
  }

  let activePointer = null;
  let rewardedForActivePointer = false;

  function isPetPetTarget(target) {
    if (!(target instanceof Element)) {
      return false;
    }
    if (target.closest(".cinder-photo-toggle")) {
      return false;
    }
    return target.closest(".pet-cinder-stage") !== null;
  }

  function updateCatHomePawPoints(pawPoints) {
    if (typeof pawPoints !== "number") {
      return;
    }

    const balance = document.querySelector(".cat-home-balance strong");
    if (balance) {
      balance.textContent = String(pawPoints);
    }
  }

  async function awardPetPet() {
    try {
      const response = await fetch("/home/cat-home/pet-pet", {
        method: "POST",
        headers: {
          Accept: "application/json",
        },
        credentials: "same-origin",
        redirect: "manual",
      });

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json().catch(() => null);
      if (!data || !data.ok || typeof data.paw_points !== "number") {
        return;
      }

      updateCatHomePawPoints(data.paw_points);
    } catch (_error) {
      // Ignore network errors; the user can tap again.
    }
  }

  petStage.addEventListener(
    "pointerdown",
    (event) => {
      if (event.button !== 0 || !isPetPetTarget(event.target)) {
        return;
      }
      activePointer = event.pointerId;
      rewardedForActivePointer = false;
    },
    { passive: true }
  );

  function clearActivePointer(pointerId) {
    if (pointerId !== activePointer) {
      return;
    }
    activePointer = null;
    rewardedForActivePointer = false;
  }

  petStage.addEventListener("pointerup", (event) => {
    clearActivePointer(event.pointerId);
  });

  petStage.addEventListener("pointercancel", (event) => {
    clearActivePointer(event.pointerId);
  });

  petStage.addEventListener("click", (event) => {
    if (event.target instanceof Element && event.target.closest(".cinder-photo-toggle")) {
      return;
    }
    if (!isPetPetTarget(event.target)) {
      return;
    }
    if (rewardedForActivePointer) {
      return;
    }

    rewardedForActivePointer = true;
    event.preventDefault();
    awardPetPet();
  });
})();
