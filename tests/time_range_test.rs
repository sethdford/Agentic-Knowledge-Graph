use chrono::{DateTime, Duration, Utc};

// A simple time range for testing
#[derive(Debug, Clone, PartialEq)]
struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        assert!(start <= end, "start time must be before or equal to end time");
        TimeRange { start, end }
    }

    /// Create a time range that starts at the given time and ends at the current time
    pub fn after(start: DateTime<Utc>) -> Self {
        TimeRange {
            start,
            end: Utc::now(),
        }
    }

    /// Create a time range that starts at Unix epoch and ends at the given time
    pub fn before(end: DateTime<Utc>) -> Self {
        TimeRange {
            // Use UNIX epoch as a very old time
            start: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
            end,
        }
    }
}

// Utility functions for temporal operations
fn temporal_overlap(range1: &TimeRange, range2: &TimeRange) -> Option<TimeRange> {
    let start = range1.start.max(range2.start);
    let end = range1.end.min(range2.end);
    
    if start <= end {
        Some(TimeRange { start, end })
    } else {
        None
    }
}

fn temporal_distance(range1: &TimeRange, range2: &TimeRange) -> Duration {
    if let Some(_) = temporal_overlap(range1, range2) {
        Duration::zero()
    } else if range1.end < range2.start {
        range2.start - range1.end
    } else {
        range1.start - range2.end
    }
}

fn is_adjacent(range1: &TimeRange, range2: &TimeRange) -> bool {
    range1.end == range2.start || range1.start == range2.end
}

#[test]
fn test_time_range() {
    // Get current time and some related time points
    let now = Utc::now();
    let before = now - Duration::hours(1);
    let after = now + Duration::hours(1);
    
    // Test creation of TimeRange and its properties
    let range1 = TimeRange::new(before, now);
    assert_eq!(range1.start, before);
    assert_eq!(range1.end, now);
    
    // Test before() constructor
    let range2 = TimeRange::before(now);
    assert_eq!(range2.start, DateTime::<Utc>::from_timestamp(0, 0).unwrap());
    assert_eq!(range2.end, now);
    
    // Test after() constructor
    let range3 = TimeRange::after(now);
    assert_eq!(range3.start, now);
    // Allow small time difference for end time (since it uses Utc::now())
    assert!(range3.end >= now);
    assert!((range3.end - now).num_seconds() < 2);
    
    // Test temporal_overlap
    let overlap = temporal_overlap(&range1, &range3).unwrap();
    assert_eq!(overlap.start, now);
    assert_eq!(overlap.end, now);
    
    // Test temporal_distance
    assert_eq!(temporal_distance(&range1, &range3), Duration::zero());
    
    let range4 = TimeRange::new(after, after + Duration::hours(1));
    let distance = temporal_distance(&range1, &range4);
    assert_eq!(distance, Duration::hours(1));
    
    let range5 = TimeRange::new(now, after);
    assert_eq!(temporal_distance(&range1, &range5), Duration::zero());
    
    // Test is_adjacent
    assert!(is_adjacent(&range1, &range3)); // They overlap at 'now', so they're adjacent
    assert!(is_adjacent(&range1, &TimeRange::new(now, after))); // End of range1 = start of new range
}

#[test]
fn test_non_overlapping_ranges() {
    let now = Utc::now();
    let two_hours_ago = now - Duration::hours(2);
    let hour_ago = now - Duration::hours(1);
    
    let range1 = TimeRange::new(two_hours_ago, hour_ago);
    let range2 = TimeRange::new(now, now + Duration::hours(1));
    
    // Ranges should not overlap
    assert!(temporal_overlap(&range1, &range2).is_none());
    
    // Distance should be 1 hour
    assert_eq!(temporal_distance(&range1, &range2), Duration::hours(1));
    
    // Ranges should not be adjacent
    assert!(!is_adjacent(&range1, &range2));
} 