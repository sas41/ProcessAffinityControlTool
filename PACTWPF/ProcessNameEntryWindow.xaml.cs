using System;
using System.Collections.Generic;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shapes;

namespace PACTWPF
{
    /// <summary>
    /// Interaction logic for ProcessNameEntryWindow.xaml
    /// </summary>
    public partial class ProcessNameEntryWindow : Window
    {
        public string ProcessName { get; set; }
        public ProcessNameEntryWindow()
        {
            InitializeComponent();
        }
        private void Button_Accept_Click(object sender, RoutedEventArgs e)
        {
            if (TextBox_ProcessName.Text.Length > 0)
            {
                this.DialogResult = true;
                ProcessName = TextBox_ProcessName.Text;
                this.Close();
            }
        }

        private void Button_Cancel_Click(object sender, RoutedEventArgs e)
        {
            this.DialogResult = false;
            this.Close();
        }

        private void TextBox_ProcessName_TextChanged(object sender, TextChangedEventArgs e)
        {
            if (TextBox_ProcessName.Text != "")
            {
                Button_Accept.IsEnabled = true;
            }
            else
            {
                Button_Accept.IsEnabled = false;
            }
        }
    }
}
