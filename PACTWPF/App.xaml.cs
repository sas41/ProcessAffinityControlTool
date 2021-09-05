using System;
using System.Collections.Generic;
using System.Configuration;
using System.Data;
using System.Linq;
using System.Threading.Tasks;
using System.Windows;

namespace PACTWPF
{
    /// <summary>
    /// Interaction logic for App.xaml
    /// </summary>
    public partial class App : Application
    {
        public bool StartMinimized { get; set; }
        private void Application_Startup(object sender, StartupEventArgs e)
        {
            if (e.Args.Length > 0)
            {
                string[] arguments = e.Args.Select(str => str.ToLower()).ToArray();
                this.StartMinimized = arguments.Contains("-start_minimized");
            }
            //base.OnStartup(e);
        }
    }
}
