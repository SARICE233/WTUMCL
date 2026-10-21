import { Box, Flex, HStack } from "@chakra-ui/react";
import { useEffect, useRef, useState } from "react";
import HomeWidgetContainer from "@/components/extension/home-widget-container";
import HomeButtonGroup from "@/components/home-button-group";
import CommunityNewsFeed from "@/components/launch/community-news-feed";
import FirstRunGuide from "@/components/launch/first-run-guide";
import LaunchGreeting from "@/components/launch/launch-greeting";
import RecentInstanceCard from "@/components/launch/recent-instance-card";
import { useGlobalData } from "@/contexts/global-data";
import styles from "@/styles/launch-home.module.css";

// Keep the information feed readable even when extension widgets are present.
const MIN_FEED_WIDTH = 320;
// Below this width a widget column is too cramped to be useful, so it is hidden
// instead of stealing the remaining space from the feed.
const MIN_WIDGET_WIDTH = 200;

const LaunchPage = () => {
  const layoutRef = useRef<HTMLDivElement>(null);
  const controlsRef = useRef<HTMLDivElement>(null);
  const [maxWidgetWidth, setMaxWidgetWidth] = useState<number | null>(null);
  const { getInstanceList } = useGlobalData();
  const instanceList = getInstanceList() || [];
  const hasInstances = instanceList.length > 0;

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
    };

    updateMaxWidgetWidth();
    const observer = new ResizeObserver(updateMaxWidgetWidth);
    observer.observe(layoutEl);
    return () => observer.disconnect();
  }, []);

  const showWidgets =
    maxWidgetWidth !== null && maxWidgetWidth >= MIN_WIDGET_WIDTH;

  return (
    <Box
      data-launch="page"
      className={styles.page}
      position="relative"
      h="100%"
      overflow="hidden"
    >
      <Box className={styles.texture} aria-hidden="true" />
      <HStack
        ref={layoutRef}
        position="relative"
        zIndex={1}
        p={7}
        pt={1}
        align="stretch"
        h="100%"
        spacing={6}
        overflow="hidden"
      >
        <Flex
          data-launch="feed"
          direction="column"
          flex="1"
          minW={0}
          minH={0}
          gap={5}
          pr={1}
          overflowY="auto"
          overflowX="hidden"
          className={styles.feed}
        >
          <LaunchGreeting />
          {hasInstances ? <RecentInstanceCard /> : <FirstRunGuide />}
          <CommunityNewsFeed />
        </Flex>

        {showWidgets && <HomeWidgetContainer maxWidth={maxWidgetWidth} />}

        <Flex
          ref={controlsRef}
          data-launch="launch-controls"
          justify="flex-end"
          align="flex-end"
          minW="14.5rem"
          flexShrink={0}
        >
          <HomeButtonGroup />
        </Flex>
      </HStack>
    </Box>
  );
};

export default LaunchPage;
