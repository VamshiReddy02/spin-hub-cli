use std::path::PathBuf;
use clap::{Parser, ArgGroup};
use itertools::Itertools;
use crate::hub_api;

#[derive(Parser, Debug)]
#[clap(about = "Create an application from a template on the Hub")]
#[clap(group(
    ArgGroup::new("operation")
        .required(true)
        .args(&["list", "name"])
))]
pub struct NewCommand {
    #[clap(short = 't')]
    terms: Vec<String>,

    #[clap(short, long)]
    list: bool,

    #[clap(name = "name", help = "Name of the application to create from the template")]
    name: Option<String>,
}

impl NewCommand {
    pub async fn run(&self) -> anyhow::Result<()> {
        if self.list {
            return self.list_templates().await;
        }

        if self.name.is_none() {
            println!("Please provide a name for the application you want to create.");
            return Ok(());
        }

        let Some(index_entry) = self.resolve_selection().await? else {
            return Ok(());
        };

        println!("Template {} by {}", index_entry.title(), index_entry.author());
        println!("{}", index_entry.summary());

        let (repo, id) = get_repo_and_id(&index_entry)?;

        self.run_template(repo, id).await
    }

    async fn list_templates(&self) -> anyhow::Result<()> {
        let entries = hub_api::index().await.unwrap();
        let matches = entries.iter()
            .filter(|e| self.is_terms_match(e))
            .sorted_by_key(|e| e.title())
            .collect_vec();

        if matches.is_empty() {
            println!("No templates match your search terms");
            return Ok(());
        }

        for entry in matches {
            println!("Template: {}\nDescription: {}\n", entry.title(), entry.summary());
        }

        Ok(())
    }

    async fn run_template(&self, repo: String, id: String) -> anyhow::Result<()> {
        use spin_templates::*;

        let manager = TemplateManager::try_default()?;

        let source = TemplateSource::try_from_git(&repo, &None, &crate::spin::version())?;
        let options = InstallOptions::default();
        manager.install(&source, &options, &DiscardingProgressReporter).await?;

        let template = manager.get(&id).unwrap().unwrap();
        let options = RunOptions {
            variant: TemplateVariantInfo::NewApplication,
            name: self.name.clone().unwrap(), 
            output_path: PathBuf::from(self.name.as_ref().unwrap()), 
            values: Default::default(),
            accept_defaults: false,
        };
        template.run(options).interactive().await
    }

    async fn resolve_selection(&self) -> Result<Option<hub_api::IndexEntry>, dialoguer::Error> {
        let entries = hub_api::index().await.unwrap();
        let matches = entries.iter().filter(|e| self.is_match(e)).sorted_by_key(|e| e.title()).collect_vec();

        match matches.len() {
            0 => {
                println!("No templates match your search terms");
                return Ok(None);
            }
            1 => {
                let index_entry = matches[0].clone();
                return Ok(Some(index_entry))
            },
            _ => {
                dialoguer::Select::new()
                    .with_prompt("Several templates match your search. Use arrow keys and Enter to select, or Esc to cancel:")
                    .items(&matches.iter().map(|e| e.title()).collect_vec())
                    .interact_opt()?
                    .map(|idx| Ok(matches[idx].clone()))
                    .transpose()
            }
        }
    }

    fn is_match(&self, index_entry: &hub_api::IndexEntry) -> bool {
        self.is_terms_match(index_entry) &&
            self.is_category_match(index_entry)
    }

    fn is_terms_match(&self, index_entry: &hub_api::IndexEntry) -> bool {
        let tags = index_entry.tags();
        let title = index_entry.title_words();
        self.terms.iter()
            .map(|t| t.to_lowercase())
            .all(|t| tags.contains(&t) || title.contains(&t))
    }

    fn is_category_match(&self, index_entry: &hub_api::IndexEntry) -> bool {
        index_entry.category() == hub_api::Category::Template
    }
}

fn get_repo_and_id(index_entry: &hub_api::IndexEntry) -> anyhow::Result<(String, String)> {
    let repo_url = index_entry.url();
    let template_id = index_entry.id(); 

    Ok((repo_url, template_id))
}

struct DiscardingProgressReporter;

impl spin_templates::ProgressReporter for DiscardingProgressReporter {
    fn report(&self, _message: impl AsRef<str>) {
    }
}
