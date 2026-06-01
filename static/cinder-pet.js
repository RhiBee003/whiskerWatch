(function () {
  const stage = document.getElementById("cinder-pet-stage");
  if (!stage) {
    return;
  }

  const photoToggle = stage.querySelector(".cinder-photo-toggle");
  const photoWrap = stage.querySelector(".pet-user-photo-optional");

  if (!photoToggle || !photoWrap) {
    return;
  }

  photoToggle.addEventListener("click", () => {
    const showPhoto = photoWrap.classList.toggle("is-visible");
    photoToggle.setAttribute("aria-pressed", showPhoto ? "true" : "false");
    photoToggle.textContent = showPhoto ? "Show Cinder" : "Show my cat photo";
    stage.classList.toggle("photo-mode", showPhoto);
  });
})();
