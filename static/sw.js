self.addEventListener("push", (event) => {
  let payload = {
    title: "WhiskerWatch",
    body: "You have a care reminder.",
    url: "/home?tab=tasks",
    tag: "whiskerwatch-reminder",
  };

  if (event.data) {
    try {
      payload = { ...payload, ...event.data.json() };
    } catch (_error) {
      payload.body = event.data.text();
    }
  }

  const options = {
    body: payload.body,
    tag: payload.tag || "whiskerwatch-reminder",
    icon: "/images/notif-cat.png",
    badge: "/images/notif-paw.png",
    data: { url: payload.url || "/home?tab=tasks" },
    requireInteraction: false,
  };

  event.waitUntil(self.registration.showNotification(payload.title, options));
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const target = event.notification.data?.url || "/home?tab=tasks";
  event.waitUntil(
    clients.matchAll({ type: "window", includeUncontrolled: true }).then((windowClients) => {
      for (const client of windowClients) {
        if ("focus" in client) {
          client.navigate(target);
          return client.focus();
        }
      }
      if (clients.openWindow) {
        return clients.openWindow(target);
      }
      return undefined;
    }),
  );
});
