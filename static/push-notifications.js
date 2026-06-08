(function initPushNotifications() {
  const enableBtn = document.getElementById("push-enable-btn");
  const statusPill = document.getElementById("push-status-pill");
  if (!enableBtn) {
    return;
  }

  const shownKey = "ww-notif-shown";
  let localTimers = [];

  function readShownTags() {
    try {
      const raw = sessionStorage.getItem(shownKey);
      return raw ? JSON.parse(raw) : {};
    } catch (_error) {
      return {};
    }
  }

  function writeShownTag(tag) {
    const shown = readShownTags();
    shown[tag] = true;
    sessionStorage.setItem(shownKey, JSON.stringify(shown));
  }

  function setStatus(text, tone) {
    if (!statusPill) {
      return;
    }
    statusPill.hidden = false;
    statusPill.textContent = text;
    statusPill.dataset.tone = tone || "neutral";
  }

  function urlBase64ToUint8Array(base64String) {
    const padding = "=".repeat((4 - (base64String.length % 4)) % 4);
    const base64 = (base64String + padding).replace(/-/g, "+").replace(/_/g, "/");
    const raw = atob(base64);
    const output = new Uint8Array(raw.length);
    for (let i = 0; i < raw.length; i += 1) {
      output[i] = raw.charCodeAt(i);
    }
    return output;
  }

  async function fetchVapidKey() {
    const response = await fetch("/push/vapid-public-key", {
      headers: { Accept: "application/json" },
      credentials: "same-origin",
    });
    if (!response.ok) {
      return null;
    }
    const data = await response.json();
    return data.public_key || null;
  }

  async function subscribeToPush() {
    if (!("serviceWorker" in navigator) || !("PushManager" in window)) {
      setStatus("This browser does not support push notifications.", "warn");
      return;
    }

    const permission = await Notification.requestPermission();
    if (permission !== "granted") {
      setStatus("Notifications blocked. Enable them in browser settings.", "warn");
      return;
    }

    const publicKey = await fetchVapidKey();
    if (!publicKey) {
      setStatus("Server push is not configured. In-tab reminders still work.", "warn");
      await refreshLocalSchedule();
      return;
    }

    const registration = await navigator.serviceWorker.register("/sw.js", { scope: "/" });
    await navigator.serviceWorker.ready;

    let subscription = await registration.pushManager.getSubscription();
    if (!subscription) {
      subscription = await registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: urlBase64ToUint8Array(publicKey),
      });
    }

    const json = subscription.toJSON();
    const response = await fetch("/home/push/subscribe", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify({
        endpoint: json.endpoint,
        p256dh: json.keys?.p256dh,
        auth: json.keys?.auth,
      }),
    });

    if (!response.ok) {
      setStatus("Could not save push subscription. Try again.", "warn");
      return;
    }

    setStatus("Push enabled on this device", "ok");
    enableBtn.textContent = "Notifications enabled";
    enableBtn.disabled = true;
    await refreshLocalSchedule();
  }

  function clearLocalTimers() {
    localTimers.forEach((timerId) => window.clearTimeout(timerId));
    localTimers = [];
  }

  function scheduleLocalReminder(reminder) {
    const shown = readShownTags();
    if (shown[reminder.tag]) {
      return;
    }

    const fireAt = new Date(reminder.at).getTime();
    const delay = fireAt - Date.now();
    if (Number.isNaN(fireAt) || delay <= 0 || delay > 24 * 60 * 60 * 1000) {
      return;
    }

    const timerId = window.setTimeout(() => {
      if (Notification.permission !== "granted") {
        return;
      }
      writeShownTag(reminder.tag);
      const notification = new Notification(reminder.title, {
        body: reminder.body,
        tag: reminder.tag,
        icon: "/images/notif-cat.png",
      });
      notification.onclick = () => {
        window.focus();
        window.location.href = reminder.url || "/home?tab=tasks";
        notification.close();
      };
    }, delay);

    localTimers.push(timerId);
  }

  async function refreshLocalSchedule() {
    clearLocalTimers();
    try {
      const response = await fetch("/home/notifications/schedule", {
        headers: { Accept: "application/json" },
        credentials: "same-origin",
      });
      if (!response.ok) {
        return;
      }
      const data = await response.json();
      (data.reminders || []).forEach(scheduleLocalReminder);
    } catch (_error) {
      // Ignore schedule fetch errors; push may still work server-side.
    }
  }

  enableBtn.addEventListener("click", () => {
    subscribeToPush().catch(() => {
      setStatus("Could not enable notifications.", "warn");
    });
  });

  if (Notification.permission === "granted") {
    navigator.serviceWorker?.getRegistration("/").then((registration) => {
      if (registration?.pushManager?.getSubscription) {
        registration.pushManager.getSubscription().then((subscription) => {
          if (subscription) {
            setStatus("Push enabled on this device", "ok");
            enableBtn.textContent = "Notifications enabled";
            enableBtn.disabled = true;
          }
        });
      }
    });
  }

  refreshLocalSchedule();
  window.setInterval(refreshLocalSchedule, 60 * 1000);
})();
