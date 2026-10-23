import { Box, HStack, Text, VStack } from "@chakra-ui/react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useLauncherConfig } from "@/contexts/config";
import { useGlobalData } from "@/contexts/global-data";

const getGreetingKey = (hour: number) => {
  if (hour >= 5 && hour < 12) return "morning";
  if (hour >= 12 && hour < 18) return "afternoon";
  if (hour >= 18 && hour < 23) return "evening";
  return "night";
};

const LaunchGreeting = ({ compact }: { compact?: boolean }) => {
  const { t, i18n } = useTranslation();
  const { config } = useLauncherConfig();
  const { selectedPlayer, getPlayerList } = useGlobalData();
  const primaryColor = config.appearance.theme.primaryColor;

  // The page is statically exported, so resolve the local time only after the
  // first paint to keep the server and client trees in sync.
  const [now, setNow] = useState<Date | null>(null);

  useEffect(() => {
    const update = () => setNow(new Date());
    update();
    const timer = window.setInterval(update, 60_000);
    return () => window.clearInterval(timer);
  }, []);

  const greetingKey = getGreetingKey(now?.getHours() ?? 12);
  // A stale/absent selection is healed by GlobalDataContext, but the first
  // player is used immediately so the greeting never falls back while waiting.
  const playerName =
    selectedPlayer?.name ||
    getPlayerList()?.[0]?.name ||
    t("LaunchPage.greeting.friend");
  const dateLocale = i18n.language === "lzh" ? "zh-Hant" : i18n.language;

  let dateLabel = "";
  if (now) {
    try {
      dateLabel = new Intl.DateTimeFormat(dateLocale, {
        weekday: "long",
        month: "long",
        day: "numeric",
      }).format(now);
    } catch {
      dateLabel = "";
    }
  }

  return (
    <VStack data-launch="greeting" align="start" spacing={1.5}>
      <HStack spacing={2} minH={4}>
        <Box w="1.75rem" h="1px" bg={`${primaryColor}.500`} />
        <Text
          fontSize="xs"
          letterSpacing="0.16em"
          textTransform="uppercase"
          className="secondary-text"
          suppressHydrationWarning
        >
          {dateLabel || "WTUMC"}
        </Text>
      </HStack>
      <Text
        as="h1"
        fontSize={compact ? "lg" : { base: "2xl", xl: "3xl" }}
        fontWeight="bold"
        lineHeight="shorter"
        noOfLines={1}
        suppressHydrationWarning
      >
        {t(`LaunchPage.greeting.${greetingKey}`, { name: playerName })}
      </Text>
      <Text fontSize="sm" className="secondary-text">
        {t("LaunchPage.greeting.subtitle")}
      </Text>
    </VStack>
  );
};

export default LaunchGreeting;
