import { HStack, Text, VStack } from "@chakra-ui/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { LuRefreshCw } from "react-icons/lu";
import AdvancedCard from "@/components/common/advanced-card";
import { CommonIconButton } from "@/components/common/common-icon-button";
import { Section } from "@/components/common/section";
import { Hitokoto } from "@/models/hitokoto";
import { DiscoverService } from "@/services/discover";

const HITOKOTO_SITE = "https://hitokoto.cn";

const HitokotoCard = () => {
  const { t } = useTranslation();
  const [hitokoto, setHitokoto] = useState<Hitokoto>();
  const [isLoading, setIsLoading] = useState(true);

  const refresh = useCallback(() => {
    setIsLoading(true);
    DiscoverService.fetchHitokoto()
      .then((response) => {
        if (response.status === "success") {
          setHitokoto(response.data);
        }
      })
      .catch(() => undefined)
      .finally(() => setIsLoading(false));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const attribution = [hitokoto?.fromWho, hitokoto?.from]
    .filter(Boolean)
    .join(" · ");

  return (
    <Section
      title={t("LaunchPage.hitokoto.title")}
      data-launch="hitokoto"
      headExtra={
        <CommonIconButton
          icon={LuRefreshCw}
          label={t("LaunchPage.hitokoto.refresh")}
          isLoading={isLoading}
          isDisabled={isLoading}
          onClick={refresh}
        />
      }
    >
      <AdvancedCard p={3}>
        <VStack align="stretch" spacing={2}>
          <Text fontSize="sm" lineHeight="tall" minH="2.5rem">
            {isLoading
              ? t("LaunchPage.hitokoto.loading")
              : hitokoto?.text || t("LaunchPage.hitokoto.empty")}
          </Text>
          <HStack spacing={2} justifyContent="space-between">
            <Text fontSize="xs" className="secondary-text" isTruncated>
              {attribution ? `— ${attribution}` : ""}
            </Text>
            <Text
              as="button"
              fontSize="xs"
              className="secondary-text"
              onClick={() => openUrl(HITOKOTO_SITE)}
            >
              hitokoto.cn
            </Text>
          </HStack>
        </VStack>
      </AdvancedCard>
    </Section>
  );
};

export default HitokotoCard;
