import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Leads, people and scores store when things happened in seconds, jobs in milliseconds.
 * Anything too small to be milliseconds (before the year 5138 in seconds) is seconds.
 */
export function toDate(timestamp: number): Date {
  return new Date(timestamp < 1e11 ? timestamp * 1000 : timestamp);
}

export function formatShortDate(timestamp: number): string {
  return toDate(timestamp).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
  });
}

export function formatLongDate(timestamp: number): string {
  return toDate(timestamp).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}
