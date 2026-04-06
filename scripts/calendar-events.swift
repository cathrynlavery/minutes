#!/usr/bin/env swift
// Queries macOS Calendar via EventKit and prints upcoming events as JSON lines.
// Usage: swift calendar-events.swift [lookahead_minutes] [lookback_minutes]
// Output: one JSON object per line:
//   {"title":"...","start":"...","minutes_until":N,"attendees":["..."],"url":"..."}
//
// When lookback_minutes is provided, queries from (now - lookback) to (now + lookahead).
// This supports overlap queries for matching recordings to calendar events.

import EventKit
import Foundation

let lookaheadMinutes = Int(CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "240") ?? 240
let lookbackMinutes = Int(CommandLine.arguments.count > 2 ? CommandLine.arguments[2] : "0") ?? 0
let store = EKEventStore()
let semaphore = DispatchSemaphore(value: 0)

store.requestFullAccessToEvents { granted, error in
    defer { semaphore.signal() }
    guard granted else {
        return
    }

    let now = Date()
    guard let end = Calendar.current.date(byAdding: .minute, value: lookaheadMinutes, to: now) else { return }
    let start: Date
    if lookbackMinutes > 0 {
        guard let s = Calendar.current.date(byAdding: .minute, value: -lookbackMinutes, to: now) else { return }
        start = s
    } else {
        start = now
    }
    let predicate = store.predicateForEvents(withStart: start, end: end, calendars: nil)
    let events = store.events(matching: predicate)
        .filter { !$0.isAllDay }
        .sorted { $0.startDate < $1.startDate }

    let formatter = DateFormatter()
    formatter.dateFormat = "yyyy-MM-dd HH:mm"

    for event in events {
        let mins = Int(event.startDate.timeIntervalSince(now) / 60)
        let startStr = formatter.string(from: event.startDate)
        let title = (event.title ?? "Untitled")
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
            .replacingOccurrences(of: "\n", with: " ")

        var attendeeNames: [String] = []
        if let attendees = event.attendees {
            for attendee in attendees {
                if let name = attendee.name {
                    let escaped = name
                        .replacingOccurrences(of: "\\", with: "\\\\")
                        .replacingOccurrences(of: "\"", with: "\\\"")
                    attendeeNames.append(escaped)
                }
            }
        }
        let attendeesJson = "[" + attendeeNames.map { "\"\($0)\"" }.joined(separator: ",") + "]"

        var urlStr = "null"
        if let location = event.location, !location.isEmpty {
            let escaped = location
                .replacingOccurrences(of: "\\", with: "\\\\")
                .replacingOccurrences(of: "\"", with: "\\\"")
            urlStr = "\"\(escaped)\""
        }

        print("{\"title\":\"\(title)\",\"start\":\"\(startStr)\",\"minutes_until\":\(mins),\"attendees\":\(attendeesJson),\"url\":\(urlStr)}")
    }
}

semaphore.wait()
