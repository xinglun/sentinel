use crate::features::research::application::gray_rhino_daily_report::GrayRhinoDailyReportRepository;
use crate::features::research::infrastructure::gray_rhino_daily_report_repository::FileGrayRhinoDailyReportRepository;
use std::path::Path;

pub(crate) fn build_gray_rhino_daily_report_repository(
    save_dir: &Path,
) -> impl GrayRhinoDailyReportRepository {
    FileGrayRhinoDailyReportRepository::new(save_dir)
}
