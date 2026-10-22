import { Box, Flex, Grid } from "@chakra-ui/react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import HomeWidgetContainer from "@/components/extension/home-widget-container";
import HomeButtonGroup from "@/components/home-button-group";
import CommunityNewsFeed from "@/components/launch/community-news-feed";
import DownloadProgressCard from "@/components/launch/download-progress-card";
import HitokotoCard from "@/components/launch/hitokoto-card";
import LaunchGreeting from "@/components/launch/launch-greeting";
import RecentInstanceCard from "@/components/launch/recent-instance-card";
import WeeklyPlaytimeCard from "@/components/launch/weekly-playtime-card";
import { useGlobalData } from "@/contexts/global-data";
import styles from "@/styles/launch-home.module.css";

// Below this available feed width the two-column layout is dropped: the feed
// takes the full width and the launch controls move under it.
const MIN_FEED_WIDTH = 320;
// Below this width a widget column is too cramped to be useful, so it is hidden
// instead of stealing the remaining space from the feed.
const MIN_WIDGET_WIDTH = 200;
// The feed stays anchored to the top-left instead of stretching on wide windows.
const FEED_MAX_WIDTH = "46rem";

const LaunchPage = () => {
  const pageRef = useRef<HTMLDivElement>(null);
  const layoutRef = useRef<HTMLDivElement>(null);
  const controlsRef = useRef<HTMLDivElement>(null);
  const [maxWidgetWidth, setMaxWidgetWidth] = useState<number | null>(null);
  const [isStacked, setIsStacked] = useState(false);
  const { getInstanceList } = useGlobalData();
  const instanceList = getInstanceList() || [];
  const hasInstances = instanceList.length > 0;

  // The shared Fade wrapper is a block box with height: 100%, so it is as tall
  // as the shell body (navbar included) instead of only the space below the
  // navbar. A page root with just flex="1" therefore falls back to its content
  // height and the bottom-right controls get clipped by the window. Pin the
  // page to a definite height: its top edge already sits below the navbar, so
  // the room left is the distance from there to the bottom of the shell body.
  useLayoutEffect(() => {
    const pageEl = pageRef.current;
    if (!pageEl) return;

    // page -> Fade (block, height: 100%) -> shell body (relative flex: 1).
    const shellEl = pageEl.parentElement?.parentElement ?? null;

    const syncPageHeight = () => {
      const pageTop = pageEl.getBoundingClientRect().top;
      const shellBottom =
        shellEl?.getBoundingClientRect().bottom ?? window.innerHeight;
      const available = Math.min(shellBottom, window.innerHeight) - pageTop;
      pageEl.style.height = `${Math.max(0, available)}px`;
    };

    syncPageHeight();

    const observer = new ResizeObserver(syncPageHeight);
    if (shellEl) {
      observer.observe(shellEl);
      const navbarEl = shellEl.firstElementChild;
      if (navbarEl) observer.observe(navbarEl);
    }
    window.addEventListener("resize", syncPageHeight);

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", syncPageHeight);
      pageEl.style.removeProperty("height");
    };
  }, []);

  useEffect(() => {
    const layoutEl = layoutRef.current;
    if (!layoutEl) return;

    const updateMaxWidgetWidth = () => {
      // main layout padding + the two stack gaps + a small safety margin
      const spacing = 112;
      const controlsWidth =
        controlsRef.current?.getBoundingClientRect().width ??
        document.getElementById("main-launch-button")?.getBoundingClientRect()
          .width ??
        14.5 * 16;
      const available =
        layoutEl.clientWidth - controlsWidth - spacing - MIN_FEED_WIDTH;
      const safeMax = Math.max(0, available);
      setMaxWidgetWidth(safeMax);
      setIsStacked(
        layoutEl.clientWidth - controlsWidth - spacing < MIN_FEED_WIDTH
      );
    };

    updateMaxWidgetWidth();
    const observer = new ResizeObserver(updateMaxWidgetWidth);
    observer.observe(layoutEl);
    return () => observer.disconnect();
  }, []);

  const showWidgets =
    !isStacked && maxWidgetWidth !== null && maxWidgetWidth >= MIN_WIDGET_WIDTH;

  return (
    <Box
      ref={pageRef}
      data-launch="page"
      className={styles.page}
      position="relative"
      display="flex"
      flexDir="column"
      flex="1"
      minH={0}
      overflow="hidden"
    >
      <Box className={styles.texture} aria-hidden="true" />
      <Box className={styles.veil} aria-hidden="true" />
      <Grid
        ref={layoutRef}
        position="relative"
        zIndex={1}
        flex="1"
        minH={0}
        p={isStacked ? 5 : 7}
        pt={1}
        gap={isStacked ? 4 : 6}
        // Wide: feed + action column. Stacked: feed on the flexible first row,
        // controls on an auto row that is never compressed or clipped.
        gridTemplateColumns={isStacked ? "minmax(0,1fr)" : "minmax(0,1fr) auto"}
        gridTemplateRows={isStacked ? "minmax(0,1fr) auto" : "minmax(0,1fr)"}
        overflow="hidden"
      >
        <Flex
          data-launch="feed"
          data-stacked={isStacked}
          direction="column"
          w="100%"
          maxW={isStacked ? "100%" : FEED_MAX_WIDTH}
          minW={0}
          minH={0}
          gap={isStacked ? 3 : 5}
          overflowY="auto"
          overflowX="hidden"
          className={styles.feed}
        >
          <LaunchGreeting compact={isStacked} />
          <WeeklyPlaytimeCard />
          {hasInstances && <RecentInstanceCard />}
          <DownloadProgressCard />
          <HitokotoCard />
          <CommunityNewsFeed />
        </Flex>

        <Flex
          direction="row"
          align="stretch"
          justify="flex-end"
          gap={isStacked ? 4 : 6}
          minW={0}
          w={isStacked ? "100%" : undefined}
        >
          {showWidgets && (
            <HomeWidgetContainer maxWidth={maxWidgetWidth ?? 0} />
          )}

          <Flex
            ref={controlsRef}
            data-launch="launch-controls"
            justify="flex-end"
            align="flex-end"
            minW={0}
            w={isStacked ? "100%" : undefined}
          >
            <HomeButtonGroup />
          </Flex>
        </Flex>
      </Grid>
    </Box>
  );
};

export default LaunchPage;
