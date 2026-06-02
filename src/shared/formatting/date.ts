const millisecondsPerDay = 24 * 60 * 60 * 1000;

export function formatTimestamp(value: string | null | undefined, emptyLabel = "Not set") {
  const trimmed = value?.trim();

  if (!trimmed) {
    return emptyLabel;
  }

  const isoTimestamp = trimmed.match(
    /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?$/,
  );

  if (isoTimestamp) {
    return `${isoTimestamp[1]} ${isoTimestamp[2]}`;
  }

  return trimmed.replace("T", " ").replace(/\.\d+$/, "");
}

export function formatLocalDate(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");

  return `${year}-${month}-${day}`;
}

export function parseLocalDate(value: string) {
  const [year, month, day] = value.split("-").map(Number);

  if (!year || !month || !day) {
    return new Date();
  }

  return new Date(year, month - 1, day);
}

export function addLocalDays(date: Date, days: number) {
  const nextDate = new Date(date);
  nextDate.setDate(nextDate.getDate() + days);

  return nextDate;
}

export function daysBetweenLocalDates(fromDate: string, toDate: string) {
  const from = parseLocalDate(fromDate).getTime();
  const to = parseLocalDate(toDate).getTime();

  return Math.round((to - from) / millisecondsPerDay);
}

export function startOfIsoWeek(date: Date) {
  const startDate = new Date(date);
  const day = startDate.getDay();
  const mondayOffset = day === 0 ? -6 : 1 - day;
  startDate.setDate(startDate.getDate() + mondayOffset);

  return startDate;
}

export function formatWeekRange(startDate: string, endDate: string) {
  return `${startDate} - ${endDate}`;
}

export function companyEventDueLabel(eventDate: string) {
  const daysUntil = daysBetweenLocalDates(formatLocalDate(new Date()), eventDate);

  if (daysUntil < 0) {
    return "Past";
  }

  if (daysUntil === 0) {
    return "Today";
  }

  if (daysUntil === 1) {
    return "Tomorrow";
  }

  if (daysUntil <= 3) {
    return `In ${daysUntil} days`;
  }

  return null;
}

export function companyEventDueClass(eventDate: string) {
  const daysUntil = daysBetweenLocalDates(formatLocalDate(new Date()), eventDate);

  if (daysUntil < 0) {
    return "event-due-past";
  }

  if (daysUntil === 0) {
    return "event-due-today";
  }

  if (daysUntil <= 3) {
    return "event-due-soon";
  }

  return "";
}

export function formatPollInterval(seconds: number) {
  if (seconds >= 86400 && seconds % 86400 === 0) {
    const days = seconds / 86400;
    return days === 1 ? "1 day" : `${days} days`;
  }

  if (seconds >= 86400) {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);

    if (hours === 0) {
      return days === 1 ? "1 day" : `${days} days`;
    }

    return `${days}d ${hours}h`;
  }

  if (seconds >= 3600) {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);

    if (minutes === 0) {
      return `${hours}h`;
    }

    return `${hours}h ${minutes}m`;
  }

  if (seconds < 60) {
    return `${seconds}s`;
  }

  if (seconds % 60 === 0) {
    return `${seconds / 60} min`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;

  return `${minutes} min ${remainingSeconds}s`;
}
