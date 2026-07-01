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

  const BOND_ACTIONS = [
    { id: "pet", label: "Gentle pet", emoji: "🐾", blurb: "+2 paw points · +10 parent XP" },
    { id: "play", label: "Playtime", emoji: "🧶", blurb: "+4 paw points · +15 parent XP" },
    { id: "cuddle", label: "Cozy cuddle", emoji: "💕", blurb: "+6 paw points · +22 parent XP" },
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

  function setBubbleText(bubble, text) {
    if (!(bubble instanceof HTMLElement)) {
      return;
    }
    const nameEl = bubble.querySelector(".cat-home-pet-bubble-name");
    if (nameEl instanceof HTMLElement) {
      nameEl.textContent = text;
      return;
    }
    bubble.textContent = text;
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
    const cats = readCats(scene);
    return (
      cats.find(
        (cat) => cat.petId === playAsId && cat.element.classList.contains("cat-home-play-as")
      ) ||
      cats.find((cat) => cat.petId === playAsId && cat.isOwned) ||
      null
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

  function readBonds(scene) {
    const dataNode = scene.querySelector(".playdate-bonds-data");
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

  function writeBonds(scene, bonds) {
    scene.querySelectorAll(".playdate-bonds-data").forEach((node) => {
      node.textContent = JSON.stringify(bonds);
    });
  }

  function bondScore(petId, bonds) {
    const match = bonds.find((entry) => entry.pet_id === petId);
    return typeof match?.score === "number" ? match.score : 0;
  }

  function findBondsPanel(scene) {
    const parent = scene.parentElement;
    if (!(parent instanceof HTMLElement)) {
      return null;
    }
    const panel = parent.querySelector(".cat-home-bonds-panel");
    return panel instanceof HTMLElement ? panel : null;
  }

  function renderBondRow(petId, petName, score) {
    const tier = friendshipTier(score);
    const percent = friendshipTierProgressPercent(score);
    const levelDisplay = formatFriendshipLevelDisplay(score);
    const row = document.createElement("li");
    row.className = "cat-home-bond-row";
    row.dataset.bondPetId = petId;
    row.innerHTML = `
      <div class="cat-home-bond-meta">
        <span class="cat-home-bond-name">${petName}</span>
        <span class="cat-home-bond-role">Your cat</span>
      </div>
      <div class="cat-home-bond-meter" role="meter" aria-valuenow="${score}" aria-valuemin="${FRIENDSHIP_SCORE_MIN}" aria-valuemax="${FRIENDSHIP_SCORE_MAX}" aria-label="Bond with ${petName}: ${tier.label} (${levelDisplay})">
        <div class="cat-home-bond-meter-fill" style="width: ${percent}%"></div>
      </div>
      <p class="cat-home-bond-tier">${tier.emoji} ${tier.label} · ${levelDisplay}</p>
    `;
    return row;
  }

  function updateBondsPanel(scene, bonds, cats) {
    const panel = findBondsPanel(scene);
    if (!panel) {
      return;
    }
    const list = panel.querySelector(".cat-home-bonds-list");
    if (!(list instanceof HTMLElement)) {
      return;
    }
    const owned = cats.filter((cat) => cat.isOwned);
    list.replaceChildren(
      ...owned.map((cat) => renderBondRow(cat.petId, cat.name, bondScore(cat.petId, bonds)))
    );
  }

  function updatePlayAsBondBadge(scene, bonds) {
    const playAs = getPlayAsCat(scene);
    if (!playAs) {
      return;
    }
    const score = bondScore(playAs.petId, bonds);
    const tier = friendshipTier(score);
    const levelDisplay = formatFriendshipLevelDisplay(score);
    const badge = playAs.element.querySelector(".cat-home-friendship-badge");
    if (badge instanceof HTMLElement) {
      badge.textContent = `${tier.emoji} ${tier.label} · ${levelDisplay}`;
    }
    playAs.element.dataset.friendshipScore = String(score);
  }

  function syncBondsAcrossScenes(bonds) {
    document.querySelectorAll(".cat-home-playdate-scene").forEach((scene) => {
      if (!(scene instanceof HTMLElement)) {
        return;
      }
      writeBonds(scene, bonds);
      const cats = readCats(scene);
      updatePlayAsBondBadge(scene, bonds);
      updateBondsPanel(scene, bonds, cats);
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

  const FRIENDSHIP_SCORE_MIN = -50;
  const FRIENDSHIP_SCORE_MAX = 100;

  function friendshipProgressPercent(score) {
    const clamped = Math.min(FRIENDSHIP_SCORE_MAX, Math.max(FRIENDSHIP_SCORE_MIN, score));
    return Math.round(((clamped - FRIENDSHIP_SCORE_MIN) * 100) / (FRIENDSHIP_SCORE_MAX - FRIENDSHIP_SCORE_MIN));
  }

  function friendshipTierFloor(score) {
    if (score <= -20) {
      return FRIENDSHIP_SCORE_MIN;
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

  function friendshipTierProgressPercent(score) {
    const floor = friendshipTierFloor(score);
    const target = friendshipNextLevelTarget(score);
    if (target <= floor) {
      return 100;
    }
    const clamped = Math.min(target, Math.max(floor, score));
    return Math.round(((clamped - floor) * 100) / (target - floor));
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
      <div class="cat-home-friendship-meter" role="meter" aria-valuenow="${score}" aria-valuemin="${FRIENDSHIP_SCORE_MIN}" aria-valuemax="${FRIENDSHIP_SCORE_MAX}" aria-label="Friendship with ${other.name}: ${tier.label} (${levelDisplay})">
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
          <p class="field-hint cat-home-friendships-lead">Bars fill toward the next friendship level shown below.</p>
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

  function addActionButtonWithHint(label, hint, onClick, className) {
    if (!(menuActions instanceof HTMLElement)) {
      return;
    }
    const button = document.createElement("button");
    button.type = "button";
    button.className = className || "download-btn playdate-action-btn";
    button.innerHTML = `<span class="playdate-action-label">${label}</span><span class="playdate-action-hint">${hint}</span>`;
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

  async function sendBondInteraction(payload) {
    const response = await fetch("/home/cat-home/bond", {
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

    let data = null;
    try {
      data = await response.json();
    } catch (_error) {
      data = null;
    }

    if (!response.ok) {
      if (response.status === 404 || response.status === 405) {
        return { ok: false, status: "route_missing" };
      }
      return data ?? { ok: false, status: "error" };
    }

    return data ?? { ok: false, status: "error" };
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
        setBubbleText(actorBubble, data.message);
      }
      if (targetBubble && !sameCat(actor, target)) {
        setBubbleText(targetBubble, `${data.friendship_emoji} ${data.friendship_label}!`);
      }
      window.setTimeout(() => {
        if (actorBubble) {
          setBubbleText(actorBubble, defaultBubbleText(actor.element));
        }
        if (targetBubble) {
          setBubbleText(targetBubble, defaultBubbleText(target.element));
        }
      }, 2000);

      const positive = !data.backfired && data.friendship_score >= 10;
      showToast(
        `${data.friendship_emoji} ${data.message} (${data.friendship_label} · ${formatFriendshipLevelDisplay(data.friendship_score)})`,
        positive
      );
      closeMenu();
    }

    async function runBondInteraction(cat, action) {
      const data = await sendBondInteraction({
        pet_id: cat.petId,
        action,
      });

      if (!data?.ok) {
        if (data?.status === "route_missing") {
          showToast("Bonding needs a quick server refresh — reload after the app restarts.");
        } else if (data?.status === "invalid_pet") {
          showToast("That cat isn't in your household yet.");
        } else {
          showToast("That cozy moment didn't save — try again.");
        }
        return;
      }

      const bonds = readBonds(scene);
      const existing = bonds.find((entry) => entry.pet_id === cat.petId);
      if (existing) {
        existing.score = data.bond_score;
      } else {
        bonds.push({ pet_id: cat.petId, score: data.bond_score });
      }
      syncBondsAcrossScenes(bonds);

      if (typeof data.paw_points === "number" && window.whiskerApplyPawPointsBalance) {
        window.whiskerApplyPawPointsBalance(data.paw_points);
      }

      pulseCats([cat.element]);

      const bubble = cat.element.querySelector(".cat-home-pet-bubble");
      if (bubble) {
        setBubbleText(bubble, data.message);
      }
      window.setTimeout(() => {
        if (bubble) {
          setBubbleText(bubble, defaultBubbleText(cat.element));
        }
      }, 2200);

      const rewardBits = [
        `+${data.paw_points_earned} paw points`,
        `+${data.parent_xp_earned} parent XP`,
      ];
      if (data.leveled_up && data.new_parent_level) {
        rewardBits.push(`Parent level ${data.new_parent_level}!`);
      }
      const positive = data.bond_score >= 10;
      showToast(
        `${data.bond_emoji} ${data.message} (${data.bond_label} · ${formatFriendshipLevelDisplay(data.bond_score)}) — ${rewardBits.join(" · ")}`,
        positive
      );
      closeMenu();
    }

    function openBondMenu(cat) {
      const bonds = readBonds(scene);
      const score = bondScore(cat.petId, bonds);
      const tier = friendshipTier(score);
      openMenu(
        `Bond with ${cat.name}`,
        `${tier.emoji} ${tier.label} · ${formatFriendshipLevelDisplay(score)} — cozy time earns paw points and parent XP.`
      );

      BOND_ACTIONS.forEach((action) => {
        addActionButtonWithHint(
          `${action.emoji} ${action.label}`,
          action.blurb,
          () => {
            runBondInteraction(cat, action.id);
          },
          "download-btn playdate-action-btn playdate-action-btn-primary"
        );
      });

      const others = sortedOthers(cat, readCats(scene));
      if (others.length) {
        addActionButton("🐱 Playdate with another cat", () => {
          clearMenu();
          openMenu(
            `${cat.name}'s playdate`,
            "Pick someone to interact with."
          );
          others.forEach((target) => {
            const friendships = readFriendships(scene);
            const targetScore = friendshipScore(cat, target, friendships);
            const targetTier = friendshipTier(targetScore);
            addActionButton(
              `${targetTier.emoji} → ${target.name} (${formatFriendshipLevelDisplay(targetScore)})`,
              () => {
                showTargetActions(cat, target, null);
              }
            );
          });
        });
      }
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

      if (playAs && sameCat(cat, playAs) && cat.isOwned) {
        openBondMenu(cat);
        return;
      }

      const friendships = readFriendships(scene);

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
      if (scene instanceof HTMLElement) {
        const bonds = readBonds(scene);
        updatePlayAsBondBadge(scene, bonds);
        updateBondsPanel(scene, bonds, readCats(scene));
      }
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
