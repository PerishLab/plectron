use super::Content;
use crate::Fault;
use crate::addons::Web;
use crate::loading::Loaded;
use crate::page::timing;
use crate::resource::Provider;
use jsruntime::Runtime;

impl Content {
    pub(super) fn start(&mut self, provider: &mut impl Provider) -> Result<(), Fault> {
        let mut loaded = Loaded::fetch(&self.url, provider)?;
        timing::mark(&mut self.milestones.resources, self.clock);
        self.loading = crate::resource::fonts::load(
            &loaded.fonts,
            provider,
            &self.fonts,
            &mut loaded.resources,
        )?;
        timing::mark(&mut self.milestones.fonts, self.clock);
        self.pictures = Some(crate::resource::Pictures::load(
            &mut loaded.document,
            &self.url,
            provider,
            &mut loaded.resources,
        )?);
        timing::mark(&mut self.milestones.pictures, self.clock);
        self.resources = loaded.resources;
        self.capture.replace(loaded.sheet.clone());
        self.version = loaded.sheet.revision();
        timing::mark(&mut self.milestones.loaded, self.clock);
        let mut web = Web::new(
            loaded.document,
            (loaded.sheet, self.capture.animations(), self.clock),
            self.config.clone(),
            &self.caches,
        )?;
        web.transport(self.url.clone(), provider.transport());
        self.runtime = Some(Runtime::new(web)?);
        timing::mark(&mut self.milestones.runtime, self.clock);
        for (source, script) in loaded.scripts {
            let runtime = self.runtime()?;
            match script {
                Some(script) => runtime.script(&script),
                None => runtime.execute(&source),
            }
            .map_err(|failure| runtime.diagnostic(&failure))?;
        }
        self.milestones
            .activation
            .get_or_insert_with(|| timing::elapsed(self.clock));
        self.render()
    }
}
