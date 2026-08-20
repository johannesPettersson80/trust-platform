export function healthStatusLabel(health: string): string {
  switch (health.trim().toLowerCase()) {
    case "connected":
      return "Connected";
    case "stopped":
      return "Stopped";
    case "starting":
      return "Starting";
    case "stopping":
      return "Stopping";
    case "unavailable":
      return "Status unavailable";
    case "configured_policy":
      return "Configured";
    case "disabled":
      return "Disabled";
    case "not_configured":
      return "Not configured";
    case "runtime_unreachable":
      return "Runtime unreachable";
    case "auth_failed":
      return "Authentication failed";
    case "degraded":
      return "Degraded";
    case "error":
      return "Error";
    case "pending":
      return "Pending";
    case "simulate":
      return "Simulator";
    case "unknown":
      return "Unknown";
    default:
      return health
        ? health.replace(/_/g, " ").replace(/\b\w/g, (char) => char.toUpperCase())
        : "Unknown";
  }
}
