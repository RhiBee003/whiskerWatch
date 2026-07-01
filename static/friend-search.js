(function () {
  function openFriendsAddCard() {
    const card = document.getElementById("friends-add-card");
    if (!(card instanceof HTMLDetailsElement)) {
      return;
    }
    card.open = true;
    card.scrollIntoView({ behavior: "smooth", block: "nearest" });
    const queryInput = document.getElementById("friend_search_query");
    if (queryInput instanceof HTMLInputElement) {
      window.setTimeout(() => queryInput.focus(), 250);
    }
  }

  document.querySelectorAll("[data-open-friends-add]").forEach((trigger) => {
    trigger.addEventListener("click", (event) => {
      event.preventDefault();
      openFriendsAddCard();
    });
  });

  const form = document.querySelector("[data-friend-search-form]");
  if (!(form instanceof HTMLFormElement)) {
    return;
  }

  const queryInput = form.querySelector("#friend_search_query");
  const resultsPanel = form.querySelector("#friend_search_results");
  const selectedPanel = form.querySelector("#friend_search_selected");
  const selectedPhoto = form.querySelector(".friend-search-selected-photo");
  const selectedName = form.querySelector(".friend-search-selected-name");
  const selectedPet = form.querySelector(".friend-search-selected-pet");
  const clearButton = form.querySelector(".friend-search-clear");
  const hiddenEmailInput = form.querySelector("#friend_email");
  const submitButton = form.querySelector("#friend_request_submit");

  if (
    !(queryInput instanceof HTMLInputElement) ||
    !(resultsPanel instanceof HTMLElement) ||
    !(selectedPanel instanceof HTMLElement) ||
    !(selectedPhoto instanceof HTMLImageElement) ||
    !(selectedName instanceof HTMLElement) ||
    !(selectedPet instanceof HTMLElement) ||
    !(clearButton instanceof HTMLButtonElement) ||
    !(hiddenEmailInput instanceof HTMLInputElement) ||
    !(submitButton instanceof HTMLButtonElement)
  ) {
    return;
  }

  let searchTimer = null;
  let activeRequest = 0;
  let selectedUser = null;

  function escapeHtml(value) {
    return String(value)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  function setResultsVisible(visible) {
    resultsPanel.hidden = !visible;
    queryInput.setAttribute("aria-expanded", visible ? "true" : "false");
  }

  function clearSelection() {
    selectedUser = null;
    hiddenEmailInput.value = "";
    submitButton.disabled = true;
    selectedPanel.hidden = true;
    queryInput.disabled = false;
    queryInput.value = "";
    queryInput.focus();
  }

  function selectUser(user) {
    selectedUser = user;
    hiddenEmailInput.value = user.email;
    submitButton.disabled = false;
    selectedPhoto.src = user.photo_url || "/cinderanimate.png";
    selectedPhoto.alt = `${user.username}'s profile photo`;
    selectedName.textContent = user.username;
    selectedPet.textContent = "WhiskerWatch cat parent";
    selectedPanel.hidden = false;
    queryInput.disabled = true;
    setResultsVisible(false);
    resultsPanel.innerHTML = "";
  }

  function renderResults(results) {
    if (!Array.isArray(results) || results.length === 0) {
      resultsPanel.innerHTML =
        '<p class="friend-search-empty">No matching usernames yet — try another spelling or keep typing. 🐾</p>';
      setResultsVisible(true);
      return;
    }

    resultsPanel.innerHTML = results
      .map((user) => {
        const photo = escapeHtml(user.photo_url || "/cinderanimate.png");
        const username = escapeHtml(user.username);
        const email = escapeHtml(user.email);
        return `<button type="button" class="friend-search-result" role="option" data-friend-email="${email}" data-friend-username="${username}" data-friend-photo="${photo}">
  <img class="friend-search-result-photo" src="${photo}" alt="" width="40" height="40" loading="lazy" />
  <span class="friend-search-result-meta">
    <strong class="friend-search-result-name">${username}</strong>
  </span>
</button>`;
      })
      .join("");
    setResultsVisible(true);
  }

  async function runSearch(query) {
    const trimmed = query.trim();
    if (!trimmed) {
      resultsPanel.innerHTML = "";
      setResultsVisible(false);
      return;
    }

    const requestId = ++activeRequest;
    try {
      const response = await fetch(`/home/friends/search?q=${encodeURIComponent(trimmed)}`, {
        headers: { Accept: "application/json" },
        credentials: "same-origin",
      });

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json();
      if (requestId !== activeRequest) {
        return;
      }

      if (!data || !data.ok) {
        resultsPanel.innerHTML =
          '<p class="friend-search-empty">Could not load matches right now. Please try again.</p>';
        setResultsVisible(true);
        return;
      }

      renderResults(data.results);
    } catch (_error) {
      if (requestId !== activeRequest) {
        return;
      }
      resultsPanel.innerHTML =
        '<p class="friend-search-empty">Could not load matches right now. Please try again.</p>';
      setResultsVisible(true);
    }
  }

  function scheduleSearch() {
    if (selectedUser) {
      return;
    }
    window.clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => {
      runSearch(queryInput.value);
    }, 220);
  }

  queryInput.addEventListener("input", scheduleSearch);
  queryInput.addEventListener("focus", () => {
    if (!selectedUser && queryInput.value.trim()) {
      scheduleSearch();
    }
  });

  resultsPanel.addEventListener("click", (event) => {
    const button = event.target instanceof Element ? event.target.closest("[data-friend-email]") : null;
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }

    selectUser({
      email: button.dataset.friendEmail || "",
      username: button.dataset.friendUsername || "",
      photo_url: button.dataset.friendPhoto || "/cinderanimate.png",
      pet_name: button.dataset.friendPet || "",
    });
  });

  clearButton.addEventListener("click", clearSelection);

  document.addEventListener("click", (event) => {
    if (!(event.target instanceof Node) || form.contains(event.target)) {
      return;
    }
    setResultsVisible(false);
  });

  form.addEventListener("submit", (event) => {
    if (!hiddenEmailInput.value.trim()) {
      event.preventDefault();
      queryInput.focus();
    }
  });
})();
