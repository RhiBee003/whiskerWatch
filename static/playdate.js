(function () {
  const menu = document.getElementById("playdate-menu");
  const menuTitle = document.getElementById("playdate-menu-title");
  const menuLead = document.getElementById("playdate-menu-lead");
  const menuActions = document.getElementById("playdate-menu-actions");
  const menuPicker = document.getElementById("playdate-menu-picker");
  const menuClose = document.querySelector(".playdate-menu-close");
  const toast = document.getElementById("playdate-toast");

  const CAT_ACTIONS = [
    { id: "sniff", label: "Curious sniff", emoji: "👃" },
    { id: "chirp", label: "Friendly chirp", emoji: "🐾" },
    { id: "groom", label: "Gentle groom", emoji: "💅" },
    { id: "friendly_brawl", label: "Friendly brawl", emoji: "🥊" },
    { id: "side_eye", label: "Side eye", emoji: "🙄" },
    { id: "hiss", label: "Dramatic hiss", emoji: "😾" },
  ];

  function friendshipTier(score) {
    if (score <= -20) {
      return { label: "Frenemies", emoji: "💢" };
    }
    if (score < 0) {
      return { label: "Wary", emoji: "😾" };
    }
    if (score < 10) {
      return { label: "Strangers", emoji: "😐" };
    }
    if (score < 30) {
      return { label: "Curious", emoji: "👀" };
    }
    if (score < 55) {
      return { label: "Acquaintances", emoji: "🐾" };
    }
    if (score < 80) {
      return { label: "Buddies", emoji: "💛" };
    }
    return { label: "Besties", emoji: "💖" };
  }

  function defaultBubbleText(catNode) {
    if (!(catNode instanceof HTMLElement)) {
      return "Cat";
    }
    const name = catNode.dataset.petName?.trim();
    return name || "Cat";
  }

  function sameCat(left, right) {
    return left.petId === right.petId && left.owner === right.owner;
  }

  function readCats(scene) {
    return Array.from(scene.querySelectorAll(".cat-home-playdate-cat"))
      .filter((node) => node instanceof HTMLElement)
      .map((node) => ({
        petId: node.dataset.petId || "",
        owner: node.dataset.petOwner || "",
        name: node.dataset.petName || "Cat",
        isOwned: node.dataset.isOwned === "true",
        isHousemate: node.dataset.isHousemate === "true",
        element: node,
      }));
  }

  function getPlayAsCat(scene) {
    const playAsId = scene.dataset.playAsPetId || "";
    if (!playAsId) {
      return null;
    }
    return (
      readCats(scene).find(
        (cat) => cat.petId === playAsId && cat.element.classList.contains("cat-home-play-as")
      ) || null
    );
  }

  function readFriendships(scene) {
    const dataNode = scene.querySelector(".playdate-friendships-data");
    if (!dataNode) {
      return [];
    }
    try {
      const parsed = JSON.parse(dataNode.textContent || "[]");
      return Array.isArray(parsed) ? parsed : [];
    } catch (_error) {
      return [];
    }
  }

  function writeFriendships(scene, friendships) {
    scene.querySelectorAll(".playdate-friendships-data").forEach((node) => {
      node.textContent = JSON.stringify(friendships);
    });
  }

  function friendshipKey(left, right) {
    const norm = (owner, petId) =>
      `${owner.trim().toLowerCase()}|${petId.trim()}`;
    const sorted = [norm(left.owner, left.petId), norm(right.owner, right.petId)].sort(
      (a, b) => a.localeCompare(b)
    );
    return `${sorted[0]}::${sorted[1]}`;
  }

  function friendshipScore(left, right, friendships) {
    const key = friendshipKey(left, right);
    const match = friendships.find((entry) => entry.key === key);
    return typeof match?.score === "number" ? match.score : 0;
  }

  function bestFriendshipForCat(cat, cats, friendships) {
    return cats
      .filter((other) => !sameCat(cat, other))
      .map((other) => friendshipScore(cat, other, friendships))
      .reduce((best, score) => Math.max(best, score), 0);
  }

  function displayFriendshipForCat(cat, cats, friendships, scene) {
    const playAs = getPlayAsCat(scene);
    if (!playAs || sameCat(cat, playAs)) {
      return bestFriendshipForCat(cat, cats, friendships);
    }
    return friendshipScore(playAs, cat, friendships);
  }

  function friendshipTierFloor(score) {
    if (score <= -20) {
      return -50;
    }
    if (score < 0) {
      return -19;
    }
    if (score < 10) {
      return 0;
    }
    if (score < 30) {
      return 10;
    }
    if (score < 55) {
      return 30;
    }
    if (score < 80) {
      return 55;
    }
    return 80;
  }

  function friendshipNextLevelTarget(score) {
    if (score <= -20) {
      return -19;
    }
    if (score < 0) {
      return 0;
    }
    if (score < 10) {
      return 10;
    }
    if (score < 30) {
      return 30;
    }
    if (score < 55) {
      return 55;
    }
    if (score < 80) {
      return 80;
    }
    return 100;
  }

  function friendshipProgressWithinTier(score, floor, ceiling) {
    if (ceiling <= floor) {
      return 100;
    }
    const clamped = Math.min(ceiling, Math.max(floor, score));
    return Math.round(((clamped - floor) * 100) / (ceiling - floor));
  }

  function friendshipTierProgressPercent(score) {
    const floor = friendshipTierFloor(score);
    const target = friendshipNextLevelTarget(score);
    return friendshipProgressWithinTier(score, floor, target);
  }

  function formatFriendshipLevelDisplay(score) {
    return `${score} / ${friendshipNextLevelTarget(score)}`;
  }

  function friendshipTargetRole(cat) {
    if (cat.isOwned) {
      return "Your housemate";
    }
    const ownerLabel = cat.element.dataset.ownerLabel?.trim();
    return ownerLabel ? `${ownerLabel}'s cat` : "Friend cat";
  }

  function findFriendshipPanel(scene) {
    const parent = scene.parentElement;
    if (!(parent instanceof HTMLElement)) {
      return null;
    }
    const panel = parent.querySelector(".cat-home-friendships-panel");
    return panel instanceof HTMLElement ? panel : null;
  }

  function renderFriendshipRow(playAs, other, friendships) {
    const score = friendshipScore(playAs, other, friendships);
    const tier = friendshipTier(score);
    const tierFloor = friendshipTierFloor(score);
    const nextTarget = friendshipNextLevelTarget(score);
    const percent = friendshipTierProgressPercent(score);
    const levelDisplay = formatFriendshipLevelDisplay(score);
    const row = document.createElement("li");
    row.className = "cat-home-friendship-row";
    if (score < 0) {
      row.dataset.scoreTier = "negative";
    }
    row.dataset.targetPetId = other.petId;
    row.dataset.targetOwner = other.owner;
    row.innerHTML = `
      <div class="cat-home-friendship-meta">
        <span class="cat-home-friendship-name">${other.name}</span>
        <span class="cat-home-friendship-role">${friendshipTargetRole(other)}</span>
      </div>
      <div class="cat-home-friendship-meter" role="meter" aria-valuenow="${score}" aria-valuemin="${tierFloor}" aria-valuemax="${nextTarget}" aria-label="Friendship with ${other.name}: ${tier.label} (${levelDisplay})">
        <div class="cat-home-friendship-meter-fill" style="width: ${percent}%"></div>
      </div>
      <p class="cat-home-friendship-tier">${tier.emoji} ${tier.label} · ${levelDisplay}</p>
    `;
    return row;
  }

  function updateFriendshipPanel(scene, friendships) {
    const panel = findFriendshipPanel(scene);
    if (!panel) {
      return;
    }

    const cats = readCats(scene);
    const playAs = getPlayAsCat(scene);
    if (cats.length < 2 || !playAs) {
      panel.classList.add("cat-home-friendships-panel--empty");
      panel.innerHTML =
        '<p class="cat-home-friendships-empty">Invite a friend\'s cat over to start tracking friendships!</p>';
      panel.setAttribute("aria-label", "Cat friendships");
      panel.removeAttribute("aria-labelledby");
      panel.removeAttribute("data-play-as-pet-id");
      return;
    }

    panel.classList.remove("cat-home-friendships-panel--empty");
    panel.dataset.playAsPetId = playAs.petId;
    panel.setAttribute("aria-labelledby", "cat-home-friendships-title");
    panel.removeAttribute("aria-label");

    let header = panel.querySelector(".cat-home-friendships-header");
    let list = panel.querySelector(".cat-home-friendships-list");
    if (!header || !list) {
      panel.innerHTML = `
        <div class="cat-home-friendships-header">
          <h3 id="cat-home-friendships-title"></h3>
          <p class="field-hint cat-home-friendships-lead">Points show progress toward the next friendship level (current / goal).</p>
        </div>
        <ul class="cat-home-friendships-list"></ul>
      `;
      header = panel.querySelector(".cat-home-friendships-header");
      list = panel.querySelector(".cat-home-friendships-list");
    }

    const title = panel.querySelector("#cat-home-friendships-title");
    if (title) {
      title.textContent = `${playAs.name}'s friendships`;
    }

    if (!(list instanceof HTMLElement)) {
      return;
    }

    const others = cats
      .filter((cat) => !sameCat(cat, playAs))
      .sort((left, right) => {
        const leftScore = friendshipScore(playAs, left, friendships);
        const rightScore = friendshipScore(playAs, right, friendships);
        if (rightScore !== leftScore) {
          return rightScore - leftScore;
        }
        return left.name.localeCompare(right.name);
      });

    list.replaceChildren(
      ...others.map((other) => renderFriendshipRow(playAs, other, friendships))
    );
  }

  function updateAllFriendshipBadges(scene, friendships) {
    const cats = readCats(scene);
    cats.forEach((cat) => {
      const score = displayFriendshipForCat(cat, cats, friendships, scene);
      const tier = friendshipTier(score);
      const levelDisplay = formatFriendshipLevelDisplay(score);
      const badge = cat.element.querySelector(".cat-home-friendship-badge");
      if (badge instanceof HTMLElement) {
        badge.textContent = `${tier.emoji} ${tier.label} · ${levelDisplay}`;
      }
      cat.element.dataset.friendshipScore = String(score);
    });
  }

  function syncFriendshipsAcrossScenes(friendships) {
    document.querySelectorAll(".cat-home-playdate-scene").forEach((scene) => {
      if (!(scene instanceof HTMLElement)) {
        return;
      }
      writeFriendships(scene, friendships);
      updateAllFriendshipBadges(scene, friendships);
      updateFriendshipPanel(scene, friendships);
    });
  }

  window.whiskerRefreshFriendshipPanel = function refreshFriendshipPanel(root) {
    const scope = root instanceof HTMLElement ? root : document;
    scope.querySelectorAll(".cat-home-playdate-scene").forEach((scene) => {
      if (!(scene instanceof HTMLElement)) {
        return;
      }
      const friendships = readFriendships(scene);
      updateAllFriendshipBadges(scene, friendships);
      updateFriendshipPanel(scene, friendships);
    });
  };

  function clearMenu() {
    if (menuActions instanceof HTMLElement) {
      menuActions.innerHTML = "";
    }
    if (menuPicker instanceof HTMLElement) {
      menuPicker.innerHTML = "";
      menuPicker.hidden = true;
    }
  }

  function closeMenu() {
    if (menu instanceof HTMLElement) {
      menu.hidden = true;
    }
    document.body.classList.remove("playdate-menu-open");
    clearMenu();
  }

  window.whiskerClosePlaydateMenu = closeMenu;

  function openMenu(title, lead) {
    if (!(menu instanceof HTMLElement)) {
      return;
    }
    if (menuTitle instanceof HTMLElement) {
      menuTitle.textContent = title;
    }
    if (menuLead instanceof HTMLElement) {
      menuLead.textContent = lead || "";
      menuLead.hidden = !lead;
    }
    clearMenu();
    menu.hidden = false;
    document.body.classList.add("playdate-menu-open");
    menuClose?.focus();
  }

  function showToast(message, isPositive) {
    if (!(toast instanceof HTMLElement)) {
      return;
    }
    toast.textContent = message;
    toast.classList.toggle("is-positive", isPositive === true);
    toast.hidden = false;
    toast.classList.remove("is-hiding");
    requestAnimationFrame(() => {
      toast.classList.add("is-visible");
    });
    window.setTimeout(() => {
      toast.classList.add("is-hiding");
      toast.classList.remove("is-visible");
      window.setTimeout(() => {
        toast.hidden = true;
        toast.classList.remove("is-hiding", "is-positive");
      }, 280);
    }, 3400);
  }

  function addActionButton(label, onClick, className) {
    if (!(menuActions instanceof HTMLElement)) {
      return;
    }
    const button = document.createElement("button");
    button.type = "button";
    button.className = className || "download-btn playdate-action-btn";
    button.textContent = label;
    button.addEventListener("click", onClick);
    menuActions.appendChild(button);
  }

  function pulseCats(elements) {
    elements.forEach((element) => {
      if (!(element instanceof HTMLElement)) {
        return;
      }
      element.classList.remove("is-playdate-react");
      void element.offsetWidth;
      element.classList.add("is-playdate-react");
      window.setTimeout(() => {
        element.classList.remove("is-playdate-react");
      }, 720);
    });
  }

  async function sendInteraction(payload) {
    const response = await fetch("/home/cat-home/playdate", {
      method: "POST",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
      },
      credentials: "same-origin",
      body: JSON.stringify(payload),
    });

    if (response.status === 401 || response.status === 403) {
      window.location.href = "/login";
      return null;
    }

    return response.json().catch(() => null);
  }

  function mountPlaydateScene(scene) {
    if (!(scene instanceof HTMLElement) || scene.dataset.playdateMounted === "true") {
      return;
    }
    scene.dataset.playdateMounted = "true";

    async function runInteraction(actor, target, action, propSlot) {
      const data = await sendInteraction({
        actor_pet_id: actor.petId,
        actor_owner: actor.owner,
        target_pet_id: target.petId,
        target_owner: target.owner,
        action,
        prop_slot: propSlot || null,
      });

      if (!data?.ok) {
        showToast("That playdate move didn't work — try again.");
        return;
      }

      const friendships = readFriendships(scene);
      const key = friendshipKey(actor, target);
      const existing = friendships.find((entry) => entry.key === key);
      if (existing) {
        existing.score = data.friendship_score;
      } else {
        friendships.push({ key, score: data.friendship_score });
      }
      syncFriendshipsAcrossScenes(friendships);

      pulseCats([actor.element, target.element]);

      const actorBubble = actor.element.querySelector(".cat-home-pet-bubble");
      const targetBubble = target.element.querySelector(".cat-home-pet-bubble");
      if (actorBubble) {
        actorBubble.textContent = data.message;
      }
      if (targetBubble && !sameCat(actor, target)) {
        targetBubble.textContent = `${data.friendship_emoji} ${data.friendship_label}!`;
      }
      window.setTimeout(() => {
        if (actorBubble) {
          actorBubble.textContent = defaultBubbleText(actor.element);
        }
        if (targetBubble) {
          targetBubble.textContent = defaultBubbleText(target.element);
        }
      }, 2000);

      const positive = !data.backfired && data.friendship_score >= 10;
      showToast(
        `${data.friendship_emoji} ${data.message} (${data.friendship_label} · ${formatFriendshipLevelDisplay(data.friendship_score)})`,
        positive
      );
      closeMenu();
    }

    function showTargetActions(actor, target, propSlot) {
      const friendships = readFriendships(scene);
      const score = friendshipScore(actor, target, friendships);
      const tier = friendshipTier(score);
      openMenu(
        `${actor.name} → ${target.name}`,
        propSlot
          ? `Pick how they play at the ${propSlot}.`
          : `${tier.emoji} ${tier.label} · ${formatFriendshipLevelDisplay(score)} — choose a playdate move.`
      );

      if (propSlot) {
        addActionButton("🎉 Play with other cat", () => {
          runInteraction(actor, target, "play_together", propSlot);
        }, "download-btn playdate-action-btn playdate-action-btn-primary");
        return;
      }

      CAT_ACTIONS.forEach((action) => {
        addActionButton(`${action.emoji} ${action.label}`, () => {
          runInteraction(actor, target, action.id, null);
        });
      });
    }

    function pickCat(title, lead, cats, onPick) {
      openMenu(title, lead);
      if (!(menuPicker instanceof HTMLElement)) {
        return;
      }
      menuPicker.hidden = false;
      cats.forEach((cat) => {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "playdate-picker-btn";
        const suffix = cat.isOwned ? "" : ` · friend`;
        button.textContent = `${cat.name}${suffix}`;
        button.addEventListener("click", () => onPick(cat));
        menuPicker.appendChild(button);
      });
    }

    function sortedOthers(fromCat, cats) {
      const friendships = readFriendships(scene);
      return cats
        .filter((other) => !sameCat(fromCat, other))
        .sort(
          (left, right) =>
            friendshipScore(fromCat, right, friendships) -
            friendshipScore(fromCat, left, friendships)
        );
    }

    function openCatMenu(cat) {
      const cats = readCats(scene);
      const playAs = getPlayAsCat(scene);
      const actor = playAs && !sameCat(playAs, cat) ? playAs : cat;
      const friendships = readFriendships(scene);

      if (sameCat(cat, playAs)) {
        const others = sortedOthers(cat, cats);
        openMenu(
          `${cat.name}'s playdate`,
          others.length
            ? "Pick someone to interact with."
            : "Invite a friend's cat to start a virtual playdate!"
        );
        others.forEach((target) => {
          const score = friendshipScore(cat, target, friendships);
          const tier = friendshipTier(score);
          addActionButton(`${tier.emoji} → ${target.name} (${formatFriendshipLevelDisplay(score)})`, () => {
            showTargetActions(cat, target, null);
          });
        });
        return;
      }

      openMenu(
        `${actor.name} → ${cat.name}`,
        cat.isHousemate
          ? "Your housemate is up for a playdate move."
          : "Choose a playdate move with this cat."
      );

      CAT_ACTIONS.forEach((action) => {
        addActionButton(`${action.emoji} ${action.label}`, () => {
          runInteraction(actor, cat, action.id, null);
        });
      });
    }

    function openPropMenu(prop) {
      const propName = prop.dataset.propName || "play spot";
      const propSlot = prop.dataset.propSlot || "";
      const cats = readCats(scene);
      const playAs = getPlayAsCat(scene);

      if (cats.length < 2) {
        openMenu(
          `Play at the ${propName}`,
          "You need at least two cats in the room for a playdate."
        );
        return;
      }

      if (playAs) {
        const remaining = sortedOthers(playAs, cats);
        if (remaining.length === 1) {
          runInteraction(playAs, remaining[0], "play_together", propSlot);
          return;
        }
        pickCat(
          `Play at the ${propName}`,
          `Who joins ${playAs.name} at the ${propName}?`,
          remaining,
          (secondCat) => {
            runInteraction(playAs, secondCat, "play_together", propSlot);
          }
        );
        return;
      }

      pickCat(
        `Play at the ${propName}`,
        "Pick the first cat, then the second.",
        cats,
        (firstCat) => {
          const remaining = cats.filter((other) => !sameCat(firstCat, other));
          pickCat(
            `Play at the ${propName}`,
            `Who plays with ${firstCat.name}?`,
            remaining,
            (secondCat) => {
              runInteraction(firstCat, secondCat, "play_together", propSlot);
            }
          );
        }
      );
    }

    scene.querySelectorAll(".cat-home-interactive").forEach((prop) => {
      if (!(prop instanceof HTMLElement)) {
        return;
      }
      prop.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        openPropMenu(prop);
      });
    });

    scene.querySelectorAll(".cat-home-playdate-cat").forEach((catNode) => {
      if (!(catNode instanceof HTMLElement)) {
        return;
      }
      catNode.addEventListener("click", (event) => {
        if (event.target instanceof Element && event.target.closest(".cinder-photo-toggle")) {
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        const cat = readCats(scene).find((entry) => entry.element === catNode);
        if (cat) {
          openCatMenu(cat);
        }
      });
      catNode.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          catNode.click();
        }
      });
    });
  }

  function mountAllPlaydateScenes() {
    document.querySelectorAll(".cat-home-playdate-scene").forEach((scene) => {
      mountPlaydateScene(scene);
    });
    window.whiskerRefreshFriendshipPanel?.();
  }

  mountAllPlaydateScenes();

  menuClose?.addEventListener("click", closeMenu);
  menu?.addEventListener("click", (event) => {
    if (event.target === menu) {
      closeMenu();
    }
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && menu instanceof HTMLElement && !menu.hidden) {
      closeMenu();
    }
  });
})();
