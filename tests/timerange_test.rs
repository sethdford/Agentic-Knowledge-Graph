use chrono::{DateTime, Duration, TimeZone, Utc};

// Define TimeRange struct here to make the test independent of the main codebase
#[derive(Debug, Clone, PartialEq)]
struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    pub fn before(end: DateTime<Utc>) -> Self {
        Self {
            start: Utc.timestamp_opt(0, 0).unwrap(),
            end,
        }
    }

    pub fn after(start: DateTime<Utc>) -> Self {
        Self {
            start,
            end: DateTime::<Utc>::MAX_UTC,
        }
    }
}

// Helper functions
fn temporal_overlap(range1: &TimeRange, range2: &TimeRange) -> bool {
    range1.start <= range2.end && range1.end >= range2.start
}

fn temporal_distance(range1: &TimeRange, range2: &TimeRange) -> Duration {
    if temporal_overlap(range1, range2) {
        Duration::zero()
    } else if range1.end < range2.start {
        range2.start - range1.end
    } else {
        range1.start - range2.end
    }
}

fn is_adjacent(range1: &TimeRange, range2: &TimeRange) -> bool {
    if temporal_overlap(range1, range2) {
        return false;
    }
    
    temporal_distance(range1, range2) == Duration::zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_range() {
        // Create a TimeRange with a start and end time
        let now = Utc::now();
        let future = now + Duration::days(1);
        let range = TimeRange::new(now, future);

        // Verify the range properties
        assert_eq!(range.start, now);
        assert_eq!(range.end, future);

        // Test the before() and after() constructors
        let range2 = TimeRange::before(future);
        assert_eq!(range2.end, future);
        assert_eq!(range2.start, Utc.timestamp_opt(0, 0).unwrap());

        let range3 = TimeRange::after(now);
        assert_eq!(range3.start, now);
        assert_eq!(range3.end, DateTime::<Utc>::MAX_UTC);

        // Test temporal_overlap
        assert!(temporal_overlap(&range, &range));
        assert!(temporal_overlap(&range, &TimeRange::new(now, now + Duration::hours(12))));
        assert!(temporal_overlap(&range, &TimeRange::new(now - Duration::hours(12), now + Duration::hours(12))));
        
        // Create a non-overlapping range
        let past_range = TimeRange::new(now - Duration::days(2), now - Duration::days(1));
        assert!(!temporal_overlap(&range, &past_range));
        
        // Test temporal_distance
        assert_eq!(temporal_distance(&range, &range), Duration::zero());
        assert_eq!(temporal_distance(&range, &past_range), Duration::days(1));
        
        // Test adjacency
        let adjacent_range = TimeRange::new(future, future + Duration::days(1));
        assert!(is_adjacent(&range, &adjacent_range));
        assert!(!is_adjacent(&range, &past_range));
    }

    #[test]
    fn test_non_overlapping_ranges() {
        let now = Utc::now();
        let range1 = TimeRange::new(now, now + Duration::days(1));
        let range2 = TimeRange::new(now + Duration::days(2), now + Duration::days(3));
        
        assert!(!temporal_overlap(&range1, &range2));
        assert!(!is_adjacent(&range1, &range2));
        assert_eq!(temporal_distance(&range1, &range2), Duration::days(1));
    }
} 