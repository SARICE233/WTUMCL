import {
  Avatar,
  Box,
  HStack,
  Icon,
  Image,
  Skeleton,
  Text,
  VStack,
} from "@chakra-ui/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useRouter } from "next/router";
import { useCallback, useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { LuChevronLeft, LuChevronRight, LuNewspaper } from "react-icons/lu";
import AdvancedCard from "@/components/common/advanced-card";
import { CommonIconButton } from "@/components/common/common-icon-button";
import Empty from "@/components/common/empty";
import { Section } from "@/components/common/section";
import { useLauncherConfig } from "@/contexts/config";
import { NewsPostSummary } from "@/models/news-post";
import { DiscoverService } from "@/services/discover";
import homeStyles from "@/styles/launch-home.module.css";
import { formatRelativeTime } from "@/utils/datetime";
import { cleanHtmlText } from "@/utils/string";

const MAX_NEWS_POSTS = 5;
// Cards shown side by side in the news row; the row slides through the rest.
const VISIBLE_NEWS_COUNT = 3;
// The row advances one card at a time on this cadence.
const NEWS_ROTATE_INTERVAL_MS = 5000;
// Keep in sync with the transform transition declared on the track rule.
const NEWS_SLIDE_DURATION_MS = 400;

const NewsCard = ({
  post,
  isActive = true,
}: {
  post: NewsPostSummary;
  // Cards waiting outside the visible window stay out of the tab order.
  isActive?: boolean;
}) => {
  const { t } = useTranslation();
  const { config } = useLauncherConfig();
  const primaryColor = config.appearance.theme.primaryColor;
  const image = Array.isArray(post.imageSrc) ? post.imageSrc[0] : undefined;

  return (
    <AdvancedCard
      data-news-card=""
      w="100%"
      minW={0}
      p={0}
      overflow="hidden"
      cursor="pointer"
      role="link"
      tabIndex={isActive ? 0 : -1}
      transition="transform 0.18s ease, box-shadow 0.18s ease"
      _hover={{
        transform: "translateY(-2px)",
        boxShadow: `0 0 0 1px var(--chakra-colors-${primaryColor}-400)`,
      }}
      _focusVisible={{
        outline: "2px solid",
        outlineColor: `${primaryColor}.400`,
        outlineOffset: "2px",
      }}
      onClick={() => openUrl(post.link)}
      onKeyDown={(event) => {
        if (event.key !== "Enter" && event.key !== " ") return;
        event.preventDefault();
        openUrl(post.link);
      }}
    >
      <Box className={homeStyles.newsImage}>
        <Box className={homeStyles.newsImageFallback}>
          <Icon as={LuNewspaper} boxSize={6} />
        </Box>
        {image && (
          <Image
            src={image}
            alt={cleanHtmlText(post.title)}
            position="absolute"
            inset={0}
            w="100%"
            h="100%"
            objectFit="cover"
            loading="lazy"
            onError={(event) => {
              event.currentTarget.style.display = "none";
            }}
          />
        )}
      </Box>
      <VStack align="stretch" spacing={1.5} p={2.5}>
        <HStack spacing={1.5} minW={0}>
          <Avatar
            size="2xs"
            name={post.source?.name}
            src={post.source?.iconSrc}
            flexShrink={0}
          />
          <Text fontSize="xs" className="secondary-text" isTruncated>
            {post.source?.name ||
              post.source?.fullName ||
              t("LaunchPage.news.title")}
          </Text>
          {post.createAt && (
            <Text
              fontSize="xs"
              className="secondary-text"
              flexShrink={0}
              ml="auto"
            >
              {formatRelativeTime(post.createAt, t)}
            </Text>
          )}
        </HStack>
        <Text fontSize="xs-sm" fontWeight="medium" noOfLines={2} minH="2.5em">
          {cleanHtmlText(post.title)}
        </Text>
      </VStack>
    </AdvancedCard>
  );
};

// The sliding row. The grid box owns the shared metrics, the viewport is the
// window that clips the horizontal overflow, and the track carries every
// rendered card and moves exactly one step per rotation.
const NewsRow = ({
  visibleCount,
  offset = 0,
  isSnapping = false,
  children,
}: {
  visibleCount: number;
  offset?: number;
  isSnapping?: boolean;
  children: ReactNode;
}) => (
  <Box
    className={homeStyles.newsGrid}
    style={{ "--news-visible": visibleCount } as CSSProperties}
  >
    <Box className={homeStyles.newsViewport}>
      <Box
        className={[
          homeStyles.newsTrack,
          isSnapping && homeStyles.newsTrackSnap,
        ]
          .filter(Boolean)
          .join(" ")}
        style={{ "--news-offset": offset } as CSSProperties}
      >
        {children}
      </Box>
    </Box>
  </Box>
);

const CommunityNewsFeed = () => {
  const { t } = useTranslation();
  const router = useRouter();
  const { config } = useLauncherConfig();

  const [posts, setPosts] = useState<NewsPostSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  // Index of the leftmost card on screen. It may briefly reach postCount while
  // the duplicated head is shown during a wrap.
  const [trackIndex, setTrackIndex] = useState(0);
  // True while the track is being moved back without a transition.
  const [isSnapping, setIsSnapping] = useState(false);
  const [isPointerInside, setIsPointerInside] = useState(false);
  const [isFocusWithin, setIsFocusWithin] = useState(false);
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(false);

  const fetchPosts = useCallback(async () => {
    setIsLoading(true);
    try {
      const sources = config.discoverSourceEndpoints
        .filter(([, enabled]) => enabled)
        .map(([url]) => ({ url, cursor: null }));

      if (sources.length === 0) {
        setPosts([]);
        return;
      }

      const response = await DiscoverService.fetchNewsPostSummaries(sources);
      setPosts(
        response.status === "success"
          ? response.data.posts.slice(0, MAX_NEWS_POSTS)
          : []
      );
    } catch {
      setPosts([]);
    } finally {
      setIsLoading(false);
    }
  }, [config.discoverSourceEndpoints]);

  useEffect(() => {
    fetchPosts();
  }, [fetchPosts]);

  // Follow the OS motion preference live, so turning it on stops the rotation
  // without a reload and turning it off starts it again.
  useEffect(() => {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const syncPreference = () => setPrefersReducedMotion(media.matches);
    syncPreference();
    media.addEventListener("change", syncPreference);
    return () => media.removeEventListener("change", syncPreference);
  }, []);

  const postCount = posts.length;
  const visibleCount = Math.min(VISIBLE_NEWS_COUNT, postCount);
  // One track carries every post, with the first visibleCount posts repeated at
  // the tail: the window can then slide from the last post onto an exact copy
  // of the first one and jump back invisibly, which makes the wrap seamless.
  const trackPosts =
    postCount > 0 ? [...posts, ...posts.slice(0, visibleCount)] : [];
  const isPaused = isPointerInside || isFocusWithin;

  // A refreshed feed starts over from the first post.
  useEffect(() => {
    setTrackIndex(0);
  }, [posts]);

  // The window landed on the duplicated head: once the slide is over, turn the
  // transition off, jump back to the real first post and let the next frame
  // re-arm it. Both frames show the same cards, so the jump is invisible and
  // the row never animates backwards through all the posts.
  useEffect(() => {
    if (postCount === 0 || trackIndex < postCount) return;
    const timer = window.setTimeout(() => {
      setIsSnapping(true);
      setTrackIndex(0);
    }, NEWS_SLIDE_DURATION_MS + 50);
    return () => window.clearTimeout(timer);
  }, [postCount, trackIndex]);

  // Re-arm the slide only after the snapped frame has been painted.
  useEffect(() => {
    if (!isSnapping) return;
    let innerFrame = 0;
    const outerFrame = window.requestAnimationFrame(() => {
      innerFrame = window.requestAnimationFrame(() => setIsSnapping(false));
    });
    return () => {
      window.cancelAnimationFrame(outerFrame);
      window.cancelAnimationFrame(innerFrame);
    };
  }, [isSnapping]);

  const showRelative = useCallback(
    (direction: -1 | 1) => {
      if (postCount <= 1) return;
      // A wrap is in flight: settle on the real track without animating, then
      // take the step from there so the row cannot jump back mid-slide.
      if (trackIndex >= postCount) {
        setIsSnapping(true);
        setTrackIndex(direction === 1 ? 1 : postCount - 1);
        return;
      }
      const nextIndex = trackIndex + direction;
      if (nextIndex < 0) {
        setTrackIndex(postCount - 1);
        return;
      }
      // With reduced motion the window is switched outright instead of slid.
      setTrackIndex(prefersReducedMotion ? nextIndex % postCount : nextIndex);
    },
    [postCount, prefersReducedMotion, trackIndex]
  );

  // Advance only while nobody is interacting with the section; hovering,
  // keyboard focus and the reduced-motion preference all hold the row still.
  useEffect(() => {
    if (prefersReducedMotion || isPaused || postCount <= 1) return;
    const timer = window.setInterval(
      () => showRelative(1),
      NEWS_ROTATE_INTERVAL_MS
    );
    return () => window.clearInterval(timer);
  }, [isPaused, postCount, prefersReducedMotion, showRelative]);

  return (
    <Section
      data-launch="community-news"
      title={t("LaunchPage.news.title")}
      headExtra={
        <HStack spacing={1}>
          <CommonIconButton
            icon={LuChevronLeft}
            label={t("LaunchPage.news.previous")}
            size="xs"
            fontSize="sm"
            h={21}
            isDisabled={postCount <= 1}
            onClick={() => showRelative(-1)}
          />
          <CommonIconButton
            icon={LuChevronRight}
            label={t("LaunchPage.news.next")}
            size="xs"
            fontSize="sm"
            h={21}
            isDisabled={postCount <= 1}
            onClick={() => showRelative(1)}
          />
          <CommonIconButton
            icon="more"
            label={t("LaunchPage.news.more")}
            size="xs"
            fontSize="sm"
            h={21}
            onClick={() => router.push("/discover/community-news")}
          />
        </HStack>
      }
      onMouseEnter={() => setIsPointerInside(true)}
      onMouseLeave={() => setIsPointerInside(false)}
      onFocusCapture={() => setIsFocusWithin(true)}
      onBlurCapture={(event) => {
        const nextTarget = event.relatedTarget as Node | null;
        if (!nextTarget || !event.currentTarget.contains(nextTarget)) {
          setIsFocusWithin(false);
        }
      }}
    >
      {isLoading ? (
        <NewsRow visibleCount={VISIBLE_NEWS_COUNT}>
          {Array.from({ length: VISIBLE_NEWS_COUNT }).map((_, index) => (
            <Box key={index} className={homeStyles.newsSlot}>
              <AdvancedCard w="100%" minW={0} p={0} overflow="hidden">
                <Skeleton h="4.75rem" w="100%" />
                <VStack align="stretch" spacing={2} p={2.5}>
                  <Skeleton h={3} w="70%" />
                  <Skeleton h={3} w="100%" />
                  <Skeleton h={3} w="80%" />
                </VStack>
              </AdvancedCard>
            </Box>
          ))}
        </NewsRow>
      ) : postCount === 0 ? (
        <Empty
          size="sm"
          withIcon={false}
          description={t("LaunchPage.news.empty")}
          py={3}
        />
      ) : (
        <NewsRow
          visibleCount={visibleCount}
          offset={trackIndex}
          isSnapping={isSnapping}
        >
          {trackPosts.map((post, slot) => {
            const isInWindow =
              slot >= trackIndex && slot < trackIndex + visibleCount;
            return (
              <Box
                key={`${post.link}-${slot}`}
                className={homeStyles.newsSlot}
                aria-hidden={isInWindow ? undefined : true}
              >
                <NewsCard post={post} isActive={isInWindow} />
              </Box>
            );
          })}
        </NewsRow>
      )}
    </Section>
  );
};

export default CommunityNewsFeed;
