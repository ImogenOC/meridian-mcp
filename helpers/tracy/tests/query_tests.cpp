#include "queries.hpp"

#include <cassert>
#include <vector>

using namespace meridian::tracy;

int main()
{
	const std::vector<ZoneStatistics> zones {
		{{"/proc/alpha", "code/a.dm", 10}, 4, 400, 250, 50, 150},
		{{"/proc/beta", "code/b.dm", 20}, 2, 600, 500, 250, 350},
		{{"/proc/alpha", "code/a2.dm", 30}, 8, 400, 300, 25, 100},
	};

	const auto hotspots = select_hotspots(zones, HotspotSort::Inclusive, 2);
	assert(hotspots.truncated);
	assert(hotspots.items.size() == 2);
	assert(hotspots.items[0].identity.name == "/proc/beta");
	assert(hotspots.items[1].identity.file == "code/a.dm");

	const auto by_count = select_hotspots(zones, HotspotSort::Count, 3);
	assert(by_count.items[0].identity.file == "code/a2.dm");

	const auto frames = summarize_frames({10, 20, 30, 40, 50, 60, 70, 80, 90, 100});
	assert(frames.count == 10);
	assert(frames.minimum == 10);
	assert(frames.maximum == 100);
	assert(frames.mean == 55);
	assert(frames.p50 == 50);
	assert(frames.p95 == 100);
	assert(frames.p99 == 100);

	const std::vector<ZoneStatistics> baseline {
		{{"/proc/alpha", "code/a.dm", 10}, 4, 400, 250, 50, 150},
		{{"/proc/beta", "code/b.dm", 20}, 2, 600, 500, 250, 350},
	};
	const std::vector<ZoneStatistics> current {
		{{"/proc/alpha", "code/a.dm", 10}, 4, 700, 500, 100, 250},
		{{"/proc/beta", "code/b.dm", 20}, 2, 500, 450, 200, 300},
		{{"/proc/new", "code/new.dm", 5}, 1, 200, 200, 200, 200},
	};
	const auto comparison = compare_zones(baseline, current, 50, 2);
	assert(comparison.truncated);
	assert(comparison.items.size() == 2);
	assert(comparison.items[0].identity.name == "/proc/alpha");
	assert(comparison.items[0].inclusive_delta == 300);
	assert(comparison.items[1].identity.name == "/proc/new");
	assert(comparison.items[1].inclusive_delta == 200);
	return 0;
}
