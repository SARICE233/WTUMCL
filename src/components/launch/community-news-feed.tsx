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
import { useCallback, useEffect, useRef, useState } from "react";
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

const NewsCard = ({ post }: { post: NewsPostSummary }) => {
  const { t } = useTranslation();
  const { config } = useLauncherConfig();
  const primaryColor = config.appearance.theme.primaryColor;
  const image = Array.isArray(post.imageSrc) ? post.imageSrc[0] : undefined;

  return (
    <AdvancedCard
      data-news-card=""
      className={homeStyles.newsCard}
      w="13.5rem"
      flexShrink={0}
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

const CommunityNewsFeed = () => {
  const { t } = useTranslation();
  const router = useRouter();
  const { config } = useLauncherConfig();

  const [posts, setPosts] = useState<NewsPostSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);
  const rowRef = useRef<HTMLDivElement>(null);

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

  const updateScrollState = useCallback(() => {
    const row = rowRef.current;
    if (!row) return;
    setCanScrollLeft(row.scrollLeft > 4);
    setCanScrollRight(row.scrollLeft + row.clientWidth < row.scrollWidth - 4);
  }, []);

  useEffect(() => {
    const frame = window.requestAnimationFrame(updateScrollState);
    return () => window.cancelAnimationFrame(frame);
  }, [posts, isLoading, updateScrollState]);

  useEffect(() => {
    const row = rowRef.current;
    if (!row) return;
    const observer = new ResizeObserver(updateScrollState);
    observer.observe(row);
    return () => observer.disconnect();
  }, [updateScrollState]);

  const scrollByPage = (direction: -1 | 1) => {
    const row = rowRef.current;
    if (!row) return;
    row.scrollBy({
      left: direction * Math.max(216, row.clientWidth * 0.8),
      behavior: "smooth",
    });
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
            isDisabled={!canScrollLeft}
            onClick={() => scrollByPage(-1)}
          />
          <CommonIconButton
            icon={LuChevronRight}
            label={t("LaunchPage.news.next")}
            size="xs"
            fontSize="sm"
            h={21}
            isDisabled={!canScrollRight}
            onClick={() => scrollByPage(1)}
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
    >
      {isLoading ? (
        <HStack spacing={3} overflow="hidden">
          {Array.from({ length: 3 }).map((_, index) => (
            <AdvancedCard
              key={index}
              w="13.5rem"
              flexShrink={0}
              p={0}
              overflow="hidden"
            >
              <Skeleton h="4.75rem" w="100%" />
              <VStack align="stretch" spacing={2} p={2.5}>
                <Skeleton h={3} w="70%" />
                <Skeleton h={3} w="100%" />
                <Skeleton h={3} w="80%" />
              </VStack>
            </AdvancedCard>
          ))}
        </HStack>
      ) : posts.length === 0 ? (
        <Empty
          size="sm"
          withIcon={false}
          description={t("LaunchPage.news.empty")}
          py={3}
        />
      ) : (
        <Box
          ref={rowRef}
          data-news-row=""
          className={homeStyles.newsRow}
          onScroll={updateScrollState}
        >
          {posts.map((post) => (
            <NewsCard key={post.link} post={post} />
          ))}
        </Box>
      )}
    </Section>
  );
};

export default CommunityNewsFeed;
