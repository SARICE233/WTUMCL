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
// The feed shows a single card at a time and advances on this cadence.
const NEWS_ROTATE_INTERVAL_MS = 5000;

const NewsCard = ({ post }: { post: NewsPostSummary }) => {
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
      tabIndex={0}
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

// Reserved, empty placeholders that keep the row three columns wide. They are
// purely decorative: no copy, no image, no icon, and nothing focusable.
const NewsSlot = () => (
  <Box
    data-launch="news-slot"
    aria-hidden="true"
    className={homeStyles.newsSlot}
  />
);

const CommunityNewsFeed = () => {
  const { t } = useTranslation();
  const router = useRouter();
  const { config } = useLauncherConfig();

  const [posts, setPosts] = useState<NewsPostSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [activeIndex, setActiveIndex] = useState(0);
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
  const currentIndex = postCount > 0 ? activeIndex % postCount : 0;
  const post = posts[currentIndex];
  const isPaused = isPointerInside || isFocusWithin;

  // Advance only while nobody is interacting with the section; hovering,
  // keyboard focus and the reduced-motion preference all hold the card still.
  useEffect(() => {
    if (prefersReducedMotion || isPaused || postCount <= 1) return;
    const timer = window.setInterval(() => {
      setActiveIndex((index) => (index + 1) % postCount);
    }, NEWS_ROTATE_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [isPaused, postCount, prefersReducedMotion]);

  const showRelative = (direction: -1 | 1) => {
    if (postCount <= 1) return;
    setActiveIndex((index) => (index + direction + postCount) % postCount);
  };

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
        <Box className={homeStyles.newsGrid}>
          <AdvancedCard w="100%" minW={0} p={0} overflow="hidden">
            <Skeleton h="4.75rem" w="100%" />
            <VStack align="stretch" spacing={2} p={2.5}>
              <Skeleton h={3} w="70%" />
              <Skeleton h={3} w="100%" />
              <Skeleton h={3} w="80%" />
            </VStack>
          </AdvancedCard>
          <NewsSlot />
          <NewsSlot />
        </Box>
      ) : !post ? (
        <Empty
          size="sm"
          withIcon={false}
          description={t("LaunchPage.news.empty")}
          py={3}
        />
      ) : (
        <Box className={homeStyles.newsGrid}>
          <NewsCard key={post.link} post={post} />
          <NewsSlot />
          <NewsSlot />
        </Box>
      )}
    </Section>
  );
};

export default CommunityNewsFeed;
